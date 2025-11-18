use crate::{
    config::ProxyConfig,
    error::{Error, Result},
    store::{KeyStore, OAuthSessionStore},
    token::TokenManager,
};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{any, get, post},
};
use jacquard_identity::JacquardResolver;
use jacquard_oauth::authstore::ClientAuthStore;
use jacquard_oauth::client::OAuthClient;
use jacquard_oauth::session::ClientData;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Main OAuth proxy server that handles both downstream (client ↔ proxy)
/// and upstream (proxy ↔ PDS) OAuth flows.
#[derive(Clone)]
pub struct OAuthProxyServer<S, K>
where
    S: OAuthSessionStore + ClientAuthStore + Clone,
    K: KeyStore + Clone,
{
    config: ProxyConfig,
    session_store: Arc<S>,
    key_store: Arc<K>,
    token_manager: Arc<TokenManager>,
    oauth_client: Arc<OAuthClient<JacquardResolver, S>>,
}

impl<S, K> OAuthProxyServer<S, K>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
{
    /// Create a new OAuth proxy server builder.
    pub fn builder() -> OAuthProxyServerBuilder<S, K> {
        OAuthProxyServerBuilder::default()
    }

    /// Create the axum router with all OAuth endpoints.
    pub fn router(&self) -> Router {
        Router::new()
            .route(
                "/.well-known/oauth-authorization-server",
                get(handle_oauth_metadata),
            )
            .route(
                "/.well-known/oauth-protected-resource",
                get(handle_protected_resource_metadata),
            )
            .route("/oauth-client-metadata.json", get(handle_client_metadata))
            .route("/oauth/jwks.json", get(handle_jwks))
            .route("/oauth/par", post(handle_par))
            .route("/oauth/authorize", get(handle_authorize))
            .route("/oauth/return", get(handle_return))
            .route("/oauth/token", post(handle_token))
            .route("/oauth/revoke", post(handle_revoke))
            .route("/xrpc/{*path}", any(handle_xrpc_proxy))
            .with_state(self.clone())
    }
}

// OAuth handler functions

/// Handle OAuth authorization server metadata discovery
async fn handle_oauth_metadata<S, K>(
    State(server): State<OAuthProxyServer<S, K>>,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
{
    let base_url = server.config.host.as_str().trim_end_matches('/');

    let metadata = serde_json::json!({
        "issuer": base_url,
        "request_parameter_supported": true,
        "request_uri_parameter_supported": true,
        "require_request_uri_registration": true,
        "scopes_supported": ["atproto", "transition:generic", "transition:chat.bsky"],
        "subject_types_supported": ["public"],
        "response_types_supported": ["code"],
        "response_modes_supported": ["query", "fragment", "form_post"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "ui_locales_supported": ["en-US"],
        "display_values_supported": ["page", "popup", "touch"],
        "authorization_response_iss_parameter_supported": true,
        "request_object_encryption_alg_values_supported": [],
        "request_object_encryption_enc_values_supported": [],
        "jwks_uri": format!("{}/oauth/jwks", base_url),
        "authorization_endpoint": format!("{}/oauth/authorize", base_url),
        "token_endpoint": format!("{}/oauth/token", base_url),
        "token_endpoint_auth_methods_supported": ["none", "private_key_jwt"],
        "revocation_endpoint": format!("{}/oauth/revoke", base_url),
        "introspection_endpoint": format!("{}/oauth/introspect", base_url),
        "pushed_authorization_request_endpoint": format!("{}/oauth/par", base_url),
        "require_pushed_authorization_requests": true,
        "client_id_metadata_document_supported": true,
        "request_object_signing_alg_values_supported": [
            "RS256", "RS384", "RS512", "PS256", "PS384", "PS512",
            "ES256", "ES256K", "ES384", "ES512", "none"
        ],
        "token_endpoint_auth_signing_alg_values_supported": [
            "RS256", "RS384", "RS512", "PS256", "PS384", "PS512",
            "ES256", "ES256K", "ES384", "ES512"
        ],
        "dpop_signing_alg_values_supported": [
            "RS256", "RS384", "RS512", "PS256", "PS384", "PS512",
            "ES256", "ES256K", "ES384", "ES512"
        ],
    });

    Ok((StatusCode::OK, Json(metadata)).into_response())
}

/// Handle OAuth protected resource metadata discovery
async fn handle_protected_resource_metadata<S, K>(
    State(server): State<OAuthProxyServer<S, K>>,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
{
    let base_url = server.config.host.as_str().trim_end_matches('/');

    let metadata = serde_json::json!({
        "resource": base_url,
        "authorization_servers": [base_url],
        "scopes_supported": [],
        "bearer_methods_supported": ["header"],
        "resource_documentation": format!("{}/xrpc", base_url),
    });

    Ok((StatusCode::OK, Json(metadata)).into_response())
}

/// ATProto OAuth Client Metadata response format
#[derive(Serialize)]
struct AtprotoClientMetadataResponse {
    client_id: String,
    application_type: String,
    grant_types: Vec<String>,
    scope: String,
    response_types: Vec<String>,
    redirect_uris: Vec<String>,
    token_endpoint_auth_method: String,
    token_endpoint_auth_signing_alg: String,
    dpop_bound_access_tokens: bool,
    jwks_uri: String,
    client_name: String,
    client_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    logo_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tos_uri: Option<String>,
}

/// Handle client metadata document (for upstream PDS)
async fn handle_client_metadata<S, K>(
    State(server): State<OAuthProxyServer<S, K>>,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
{
    let metadata = &server.config.client_metadata;

    // Convert scopes array to space-separated string
    let scope_string = metadata
        .scopes
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join(" ");

    // Convert grant types to strings
    let grant_types = metadata
        .grant_types
        .iter()
        .map(|gt| match gt {
            jacquard_oauth::atproto::GrantType::AuthorizationCode => "authorization_code",
            jacquard_oauth::atproto::GrantType::RefreshToken => "refresh_token",
            _ => "unknown",
        })
        .map(String::from)
        .collect();

    let response = AtprotoClientMetadataResponse {
        client_id: metadata.client_id.to_string(),
        application_type: "web".to_string(),
        grant_types,
        scope: scope_string,
        response_types: vec!["code".to_string()],
        redirect_uris: metadata
            .redirect_uris
            .iter()
            .map(|u| u.to_string())
            .collect(),
        token_endpoint_auth_method: "private_key_jwt".to_string(),
        token_endpoint_auth_signing_alg: "ES256".to_string(),
        dpop_bound_access_tokens: true,
        jwks_uri: metadata
            .jwks_uri
            .as_ref()
            .map(|u| u.to_string())
            .unwrap_or_default(),
        client_name: metadata
            .client_name
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_default(),
        client_uri: metadata
            .client_uri
            .as_ref()
            .map(|u| u.to_string())
            .unwrap_or_default(),
        logo_uri: metadata.logo_uri.as_ref().map(|u| u.to_string()),
        tos_uri: metadata.tos_uri.as_ref().map(|u| u.to_string()),
    };

    Ok((StatusCode::OK, Json(response)).into_response())
}

/// Handle JWKS endpoint (public keys for JWT verification)
async fn handle_jwks<S, K>(State(server): State<OAuthProxyServer<S, K>>) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
{
    use base64::Engine;
    use p256::elliptic_curve::sec1::ToEncodedPoint;

    let signing_key = server.key_store.get_signing_key().await?;
    let verifying_key = signing_key.verifying_key();
    let encoded_point = verifying_key.to_encoded_point(false);

    let x = encoded_point
        .x()
        .ok_or_else(|| Error::InvalidRequest("missing x coordinate".to_string()))?;
    let y = encoded_point
        .y()
        .ok_or_else(|| Error::InvalidRequest("missing y coordinate".to_string()))?;

    // Construct JWKS manually - standard JSON format for JWK Set
    let jwks = serde_json::json!({
        "keys": [{
            "kty": "EC",
            "crv": "P-256",
            "x": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(x.as_slice()),
            "y": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(y.as_slice()),
            "use": "sig",
            "alg": "ES256",
            "kid": "proxy-signing-key"
        }]
    });

    Ok((StatusCode::OK, Json(jwks)).into_response())
}

/// Handle Pushed Authorization Request (PAR).
async fn handle_par<S, K>(
    State(server): State<OAuthProxyServer<S, K>>,
    headers: HeaderMap,
    body: String,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
{
    tracing::info!("handling PAR request");

    // Extract and parse DPoP proof
    let dpop_proof_str = headers
        .get("DPoP")
        .and_then(|v| v.to_str().ok())
        .ok_or(Error::DpopProofRequired)?;

    // Get HTTP method and URL for DPoP validation
    let http_method = "POST";
    let http_uri = format!("{}/oauth/par", server.config.host);

    // Parse the PAR parameters - try JSON first, then form-encoded
    let params: PARRequest = if let Some(content_type) = headers.get("content-type") {
        if content_type
            .to_str()
            .unwrap_or("")
            .contains("application/json")
        {
            serde_json::from_str(&body)
                .map_err(|e| Error::InvalidRequest(format!("invalid JSON: {}", e)))?
        } else {
            serde_urlencoded::from_str(&body)
                .map_err(|e| Error::InvalidRequest(format!("invalid form data: {}", e)))?
        }
    } else {
        // Default to JSON if no content-type
        serde_json::from_str(&body)
            .or_else(|_| serde_urlencoded::from_str(&body))
            .map_err(|e| Error::InvalidRequest(format!("invalid request body: {}", e)))?
    };

    // Validate required parameters
    if params.client_id.is_empty() {
        return Err(Error::InvalidRequest("missing client_id".to_string()));
    }
    if params.redirect_uri.is_empty() {
        return Err(Error::InvalidRequest("missing redirect_uri".to_string()));
    }
    if params.code_challenge.is_none() {
        return Err(Error::InvalidRequest("missing code_challenge".to_string()));
    }
    if params.code_challenge_method.as_deref() != Some("S256") {
        return Err(Error::InvalidRequest(
            "only S256 code_challenge_method supported".to_string(),
        ));
    }

    // Configure DPoP verification with HMAC-based nonces
    // The nonces are stateless and bound to the client
    let hmac_config = dpop_verifier::HmacConfig::new(
        &server.config.dpop_nonce_hmac_secret,
        300,  // 5 minute max age
        true, // bind to HTU/HTM
        true, // bind to JKT
        true, // bind to client
    );

    // Create a simple in-memory replay store for this request
    let mut replay_store = SimpleReplayStore::new(server.session_store.clone());

    // Verify the DPoP proof using builder pattern
    let verifier = dpop_verifier::DpopVerifier::new()
        .with_max_age_seconds(300)
        .with_future_skew_seconds(5)
        .with_nonce_mode(dpop_verifier::NonceMode::Hmac(hmac_config))
        .with_client_binding(params.client_id.clone());

    let verified = verifier
        .verify(
            &mut replay_store,
            dpop_proof_str,
            &http_uri,
            http_method,
            None, // no access token for PAR
        )
        .await
        .map_err(|e| match e {
            dpop_verifier::DpopError::UseDpopNonce { nonce } => {
                // Return a special error that includes the nonce
                // The caller will need to return this as a DPoP-Nonce header
                Error::DpopNonceRequired(nonce)
            }
            _ => Error::InvalidRequest(format!("invalid DPoP proof: {}", e)),
        })?;

    let downstream_dpop_jkt = verified.jkt;

    tracing::info!("validated DPoP proof with JKT: {}", downstream_dpop_jkt);
    tracing::info!("PAR request state: {:?}", params.state);

    // Check if we have an existing session for this JKT
    let _session_id = if let Some(session) = server
        .session_store
        .get_by_dpop_jkt(&downstream_dpop_jkt)
        .await?
    {
        tracing::info!("found existing session for JKT: {}", session.id);
        session.id
    } else {
        tracing::info!("no existing session found, creating new session for JKT");
        let session_id = generate_random_string(32);
        tracing::info!("created new session: {}", session_id);
        session_id
    };

    tracing::info!(
        "validated PAR parameters for client_id: {}",
        params.client_id
    );

    // Generate request_uri
    let request_uri = format!(
        "urn:ietf:params:oauth:request_uri:{}",
        generate_random_string(32)
    );

    // Store PAR data with 90 second expiry (per spec)
    let par_data = crate::store::PARData {
        client_id: params.client_id,
        redirect_uri: params.redirect_uri,
        response_type: params.response_type,
        state: params.state,
        scope: params.scope,
        code_challenge: params.code_challenge,
        code_challenge_method: params.code_challenge_method,
        login_hint: params.login_hint,
        downstream_dpop_jkt: downstream_dpop_jkt.clone(),
        expires_at: chrono::Utc::now() + chrono::Duration::seconds(90),
    };

    server
        .session_store
        .store_par_data(&request_uri, par_data.clone())
        .await?;

    // Store downstream client info keyed by JKT
    // This will be retrieved in the callback after we look up the session
    let downstream_info = crate::store::DownstreamClientInfo {
        redirect_uri: par_data.redirect_uri,
        state: par_data.state,
        response_type: par_data.response_type,
        scope: par_data.scope,
        expires_at: par_data.expires_at,
    };

    server
        .session_store
        .store_downstream_client_info(&downstream_dpop_jkt, downstream_info)
        .await?;

    tracing::info!(
        "stored PAR data with request_uri: {} and client info for downstream JKT: {}",
        request_uri,
        downstream_dpop_jkt
    );

    // Return response with request_uri and the JKT in the session
    // The JKT will be retrieved from PAR data in the authorize handler
    let response = serde_json::json!({
        "request_uri": request_uri,
        "expires_in": 90
    });

    Ok((StatusCode::CREATED, Json(response)).into_response())
}

/// Handle authorization request - redirect to upstream PDS.
async fn handle_authorize<S, K>(
    State(server): State<OAuthProxyServer<S, K>>,
    Query(params): Query<AuthorizeParams>,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
{
    tracing::info!("handling authorize request");

    // If request_uri is provided, retrieve PAR data
    let (client_id, redirect_uri, response_type, state, scope, login_hint, _downstream_dpop_jkt) =
        if let Some(ref request_uri) = params.request_uri {
            tracing::info!("using PAR request_uri: {}", request_uri);

            let par_data = server
                .session_store
                .consume_par_data(request_uri)
                .await?
                .ok_or_else(|| {
                    Error::InvalidRequest("invalid or expired request_uri".to_string())
                })?;

            // Check expiry
            if par_data.expires_at < chrono::Utc::now() {
                return Err(Error::InvalidRequest("request_uri expired".to_string()));
            }

            (
                par_data.client_id,
                par_data.redirect_uri,
                par_data.response_type,
                par_data.state,
                par_data.scope,
                par_data.login_hint,
                Some(par_data.downstream_dpop_jkt),
            )
        } else {
            // Use parameters from query string
            (
                params
                    .client_id
                    .ok_or_else(|| Error::InvalidRequest("missing client_id".to_string()))?,
                params
                    .redirect_uri
                    .ok_or_else(|| Error::InvalidRequest("missing redirect_uri".to_string()))?,
                params
                    .response_type
                    .ok_or_else(|| Error::InvalidRequest("missing response_type".to_string()))?,
                params.state,
                params.scope,
                None, // no login_hint in direct authorize
                None, // no JKT in direct authorize
            )
        };

    tracing::info!("handling authorize request for client_id: {}", client_id);

    // Get the user identifier from login_hint
    let user_identifier =
        login_hint.ok_or_else(|| Error::InvalidRequest("missing login_hint".to_string()))?;

    // Use jacquard OAuth client to start upstream auth flow
    // This will resolve the PDS, create PAR, and return the authorization URL
    // Generate our own state to link upstream and downstream flows
    let proxy_state = generate_random_string(32);

    // Parse the scope from the client request
    let requested_scopes: Vec<jacquard_oauth::scopes::Scope> = scope
        .as_ref()
        .map(|s| {
            s.split_whitespace()
                .filter_map(|scope_str| scope_str.parse().ok())
                .collect()
        })
        .unwrap_or_else(|| server.config.scope.clone());

    tracing::info!("got scopes {:?}", requested_scopes);
    tracing::info!(
        "starting upstream auth for user_identifier: {}",
        user_identifier
    );

    let auth_options = jacquard_oauth::types::AuthorizeOptions {
        scopes: requested_scopes,
        state: Some(proxy_state.clone().into()),
        ..Default::default()
    };

    tracing::info!("calling start_auth with options: state={}", proxy_state);
    let auth_url = server
        .oauth_client
        .start_auth(&user_identifier, auth_options)
        .await
        .map_err(|e| {
            tracing::error!("start_auth failed: {}", e);
            Error::InvalidRequest(format!("failed to start auth: {}", e))
        })?;

    // Store downstream client info by proxy_state
    // When callback returns with this state, we can retrieve the client info directly
    let downstream_info = crate::store::DownstreamClientInfo {
        redirect_uri: redirect_uri.clone(),
        state: state.clone(),
        response_type: response_type.clone(),
        scope: scope.clone(),
        expires_at: chrono::Utc::now() + chrono::Duration::minutes(10),
    };

    server
        .session_store
        .store_downstream_client_info(&proxy_state, downstream_info)
        .await?;

    tracing::info!(
        "stored downstream client info for proxy_state: {}",
        proxy_state
    );
    tracing::info!("redirecting to upstream PDS auth: {}", auth_url);
    Ok(Redirect::to(&auth_url).into_response())
}

/// Handle OAuth callback from upstream PDS.
async fn handle_return<S, K>(
    State(server): State<OAuthProxyServer<S, K>>,
    Query(params): Query<CallbackParams>,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
{
    tracing::info!("handling OAuth callback with params: {:?}", params);

    // Check for errors from upstream PDS
    if let Some(error) = params.error {
        tracing::error!("upstream auth error: {}", error);
        return Err(Error::InvalidRequest(format!(
            "upstream auth failed: {}",
            error
        )));
    }

    let code = params.code.as_deref().ok_or_else(|| {
        tracing::error!("missing code in callback");
        Error::InvalidGrant
    })?;

    let state = params.state.as_deref().ok_or_else(|| {
        tracing::error!("missing state in callback, params: {:?}", params);
        Error::InvalidRequest("missing state".to_string())
    })?;

    // Exchange authorization code for upstream tokens using jacquard-oauth
    let callback_params = jacquard_oauth::types::CallbackParams {
        code: code.into(),
        state: Some(state.into()),
        iss: params.iss.as_deref().map(|s| s.into()),
    };

    let oauth_session = server
        .oauth_client
        .callback(callback_params)
        .await
        .map_err(|e| {
            tracing::error!("callback failed with error: {}", e);
            Error::InvalidRequest(format!("failed to exchange code: {}", e))
        })?;

    // Extract session data
    let session_data = oauth_session.data.read().await;
    let account_did = session_data.account_did.to_string();
    let _pds_url = session_data.host_url.to_string();
    let upstream_session_id = session_data.session_id.to_string();

    // Get the DPoP key from dpop_data
    let dpop_key = session_data.dpop_data.dpop_key.clone();
    drop(session_data); // release the read lock

    tracing::info!(
        "successfully exchanged code for upstream tokens, DID: {}, session_id: {}",
        account_did,
        upstream_session_id
    );

    // Store the upstream DPoP key for this session
    // Serialize and deserialize to convert jose_jwk::Key to jose_jwk::Jwk
    let dpop_key_json = serde_json::to_value(&dpop_key)
        .map_err(|e| Error::InvalidRequest(format!("failed to serialize DPoP key: {}", e)))?;
    let dpop_jwk: jose_jwk::Jwk = serde_json::from_value(dpop_key_json)
        .map_err(|e| Error::InvalidRequest(format!("failed to parse DPoP key: {}", e)))?;

    let dpop_jkt = compute_jwk_thumbprint(&dpop_jwk)?;
    server
        .session_store
        .store_session_dpop_key(&upstream_session_id, dpop_jkt, dpop_jwk)
        .await?;

    tracing::info!("stored upstream DPoP key for session");

    // Retrieve downstream client info using the proxy_state
    let downstream_client_info = server
        .session_store
        .consume_downstream_client_info(state)
        .await?
        .ok_or_else(|| {
            tracing::error!("no client info found for state: {}", state);
            Error::InvalidRequest("session not found".to_string())
        })?;

    tracing::info!(
        "retrieved downstream client info, redirecting to: {}",
        downstream_client_info.redirect_uri
    );

    // Generate a downstream authorization code for the client
    let downstream_code = generate_random_string(32);

    // Store the pending auth so we can exchange it for tokens later
    let pending_auth = crate::store::PendingAuth {
        account_did,
        upstream_session_id,
        redirect_uri: downstream_client_info.redirect_uri.clone(),
        state: downstream_client_info.state.clone(),
        expires_at: chrono::Utc::now() + chrono::Duration::minutes(10),
    };

    server
        .session_store
        .store_pending_auth(&downstream_code, pending_auth.clone())
        .await?;

    // Redirect back to the client with the downstream authorization code
    // Use hash fragment instead of query params (OAuth implicit flow style)
    // Include iss (issuer) parameter for security
    let issuer = server.config.host.to_string();
    let issuer = issuer.trim_end_matches('/');
    let redirect_url = format!(
        "{}#code={}&state={}&iss={}",
        pending_auth.redirect_uri,
        urlencoding::encode(&downstream_code),
        urlencoding::encode(&pending_auth.state.as_deref().unwrap_or("")),
        urlencoding::encode(issuer)
    );

    tracing::info!("redirecting client to: {}", redirect_url);

    Ok(Redirect::to(&redirect_url).into_response())
}

/// Handle token request (exchange code for tokens or refresh tokens).
async fn handle_token<S, K>(
    State(server): State<OAuthProxyServer<S, K>>,
    headers: HeaderMap,
    body: String,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
{
    tracing::info!("handling token request");

    // Parse token request - try JSON first, then form-encoded
    let params: TokenRequest = if let Some(content_type) = headers.get("content-type") {
        if content_type
            .to_str()
            .unwrap_or("")
            .contains("application/json")
        {
            serde_json::from_str(&body)
                .map_err(|e| Error::InvalidRequest(format!("invalid JSON: {}", e)))?
        } else {
            serde_urlencoded::from_str(&body)
                .map_err(|e| Error::InvalidRequest(format!("invalid form data: {}", e)))?
        }
    } else {
        // Default to form-encoded if no content-type
        serde_urlencoded::from_str(&body)
            .map_err(|e| Error::InvalidRequest(format!("invalid request body: {}", e)))?
    };

    match params.grant_type.as_str() {
        "authorization_code" => {
            let code = params
                .code
                .ok_or_else(|| Error::InvalidRequest("missing code".to_string()))?;

            // Extract client's DPoP JKT
            let dpop_jkt = extract_dpop_jkt(&headers)?;

            // Look up and consume the pending auth
            let pending_auth = server
                .session_store
                .consume_pending_auth(&code)
                .await?
                .ok_or_else(|| Error::InvalidGrant)?;

            tracing::info!(
                "exchanging downstream code for DID: {}",
                pending_auth.account_did
            );

            // Get the upstream session from jacquard-oauth store
            let did = jacquard_common::types::did::Did::new_owned(&pending_auth.account_did)
                .map_err(|e| Error::InvalidRequest(format!("invalid DID: {}", e)))?;
            let upstream_session_data = ClientAuthStore::get_session(
                &*server.session_store,
                &did,
                &pending_auth.upstream_session_id,
            )
            .await
            .map_err(|e| Error::InvalidRequest(format!("failed to get session: {}", e)))?
            .ok_or_else(|| Error::SessionNotFound)?;

            let scope_str = upstream_session_data
                .token_set
                .scope
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    server
                        .config
                        .scope
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                });

            tracing::info!(
                "upstream token scope: {}, issuing downstream JWT with same scope",
                scope_str
            );

            // Issue downstream JWT bound to client's DPoP key
            let access_token = server
                .token_manager
                .issue_downstream_jwt(
                    &pending_auth.account_did,
                    &dpop_jkt,
                    &scope_str,
                    server.config.downstream_token_expiry_seconds,
                    &*server.key_store,
                )
                .await?;

            // Generate downstream refresh token (separate from upstream)
            let downstream_refresh_token = generate_random_string(64);

            // Store mapping: downstream_refresh_token → (account_did, upstream_session_id)
            server
                .session_store
                .store_refresh_token_mapping(
                    &downstream_refresh_token,
                    pending_auth.account_did.clone(),
                    pending_auth.upstream_session_id.clone(),
                )
                .await?;

            tracing::info!(
                "issued downstream JWT and refresh token for DID: {}",
                pending_auth.account_did
            );

            // Store the session so XRPC proxy can look it up
            // We already have the complete upstream_session_data, just store it
            ClientAuthStore::upsert_session(&*server.session_store, upstream_session_data.clone())
                .await
                .map_err(|e| Error::InvalidRequest(format!("failed to store session: {}", e)))?;

            // Also store the active session mapping (DID → session_id)
            server
                .session_store
                .store_active_session(
                    &pending_auth.account_did,
                    pending_auth.upstream_session_id.clone(),
                )
                .await?;

            tracing::info!(
                "stored session for DID: {}, session_id: {}",
                pending_auth.account_did,
                pending_auth.upstream_session_id
            );

            let response = TokenResponse {
                access_token,
                token_type: "DPoP".to_string(),
                expires_in: server.config.downstream_token_expiry_seconds as u64,
                refresh_token: Some(downstream_refresh_token),
                scope: scope_str,
                sub: pending_auth.account_did.clone(),
            };

            Ok(Json(response).into_response())
        }
        "refresh_token" => {
            let refresh_token = params
                .refresh_token
                .ok_or_else(|| Error::InvalidRequest("missing refresh_token".to_string()))?;

            // Extract client's DPoP JKT (may have changed)
            let dpop_jkt = extract_dpop_jkt(&headers)?;

            tracing::info!("handling refresh token request");

            // Look up the session by refresh token
            let (account_did, session_id) = server
                .session_store
                .get_refresh_token_mapping(&refresh_token)
                .await?
                .ok_or_else(|| Error::InvalidGrant)?;

            tracing::info!("refreshing token for DID: {}", account_did);

            // Get the upstream session from jacquard-oauth store
            let did = jacquard_common::types::did::Did::new_owned(&account_did)
                .map_err(|e| Error::InvalidRequest(format!("invalid DID: {}", e)))?;
            let upstream_session_data =
                ClientAuthStore::get_session(&*server.session_store, &did, &session_id)
                    .await
                    .map_err(|e| Error::InvalidRequest(format!("failed to get session: {}", e)))?
                    .ok_or_else(|| Error::SessionNotFound)?;

            // jacquard-oauth handles token refresh automatically when the session is accessed
            let scope_str = upstream_session_data
                .token_set
                .scope
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    server
                        .config
                        .scope
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                });

            // Issue new downstream JWT
            let access_token = server
                .token_manager
                .issue_downstream_jwt(
                    &account_did,
                    &dpop_jkt,
                    &scope_str,
                    server.config.downstream_token_expiry_seconds,
                    &*server.key_store,
                )
                .await?;

            // Generate new downstream refresh token (token rotation)
            let new_downstream_refresh = generate_random_string(64);

            // Update mapping
            server
                .session_store
                .store_refresh_token_mapping(
                    &new_downstream_refresh,
                    account_did.clone(),
                    session_id.clone(),
                )
                .await?;

            tracing::info!(
                "issued new downstream JWT and refresh token for DID: {}",
                account_did
            );

            // Store/update the session (we already have the complete upstream_session_data)
            ClientAuthStore::upsert_session(&*server.session_store, upstream_session_data.clone())
                .await
                .map_err(|e| Error::InvalidRequest(format!("failed to store session: {}", e)))?;

            // Also store the active session mapping (DID → session_id)
            server
                .session_store
                .store_active_session(&account_did, session_id.clone())
                .await?;

            let response = TokenResponse {
                access_token,
                token_type: "DPoP".to_string(),
                expires_in: server.config.downstream_token_expiry_seconds as u64,
                refresh_token: Some(new_downstream_refresh),
                scope: scope_str,
                sub: account_did,
            };

            Ok(Json(response).into_response())
        }
        _ => Err(Error::InvalidGrant),
    }
}

/// Handle token revocation.
async fn handle_revoke<S, K>(
    State(server): State<OAuthProxyServer<S, K>>,
    headers: HeaderMap,
    _body: String,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone,
    K: KeyStore + Clone,
{
    tracing::info!("handling revoke request");

    // Extract DPoP JKT
    let dpop_jkt = extract_dpop_jkt(&headers)?;

    // Look up and delete the session
    let session = server
        .session_store
        .get_by_dpop_jkt(&dpop_jkt)
        .await?
        .ok_or(Error::SessionNotFound)?;

    OAuthSessionStore::delete_session(&*server.session_store, &session.id).await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Proxy XRPC requests to the user's PDS with authenticated context.
async fn handle_xrpc_proxy<S, K>(
    State(server): State<OAuthProxyServer<S, K>>,
    method: Method,
    uri: http::Uri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
{
    tracing::info!("proxying XRPC request: {} {}", method, uri.path());

    // 1. Extract and validate downstream JWT from Authorization header
    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(Error::Unauthorized)?;

    let token = auth_header
        .strip_prefix("DPoP ")
        .or_else(|| auth_header.strip_prefix("Bearer "))
        .ok_or(Error::Unauthorized)?;

    let claims = server
        .token_manager
        .validate_downstream_jwt(token, &*server.key_store)
        .await?;

    tracing::info!("validated JWT for DID: {}", claims.sub);

    // 2. Verify DPoP binding
    let dpop_jkt = extract_dpop_jkt(&headers)?;
    if dpop_jkt != claims.cnf.jkt {
        return Err(Error::InvalidRequest("DPoP key mismatch".to_string()));
    } else {
        tracing::info!("DPoP key binding verified");
    }

    tracing::info!("Looking up active session for sub: {}", &claims.sub);
    // 3. Look up active session for this user
    let session_id = server
        .session_store
        .get_active_session(&claims.sub)
        .await?
        .ok_or(Error::SessionNotFound)?;

    let did = jacquard_common::types::did::Did::new_owned(&claims.sub)
        .map_err(|e| Error::InvalidRequest(format!("invalid DID: {}", e)))?;

    tracing::info!(
        "getting session for DID {} and session_id {}",
        &did,
        &session_id
    );
    let upstream_session_data =
        ClientAuthStore::get_session(&*server.session_store, &did, &session_id)
            .await
            .map_err(|e| Error::InvalidRequest(format!("failed to get session: {}", e)))?
            .ok_or(Error::SessionNotFound)?;

    tracing::info!("found upstream session for DID: {}", claims.sub);

    // 4. Build upstream URL
    let path = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("");
    let host_url = upstream_session_data
        .host_url
        .as_str()
        .trim_end_matches('/');
    let path = path.trim_start_matches('/');
    let upstream_url = format!("{}/{}", host_url, path);

    tracing::info!("upstream URL: {}", upstream_url);

    // 5. Get upstream DPoP key
    let upstream_dpop_key = server
        .session_store
        .get_session_dpop_key(&session_id)
        .await?
        .map(|(_jkt, key)| key)
        .ok_or_else(|| Error::InvalidRequest("DPoP key not found for session".to_string()))?;

    tracing::info!("retrieved DPoP key for upstream request");

    // 6. Get stored DPoP nonce (if any)
    let mut dpop_nonce = server
        .session_store
        .get_session_dpop_nonce(&session_id)
        .await?;

    // Retry loop for DPoP nonce handling
    let mut retry_count = 0;
    let max_retries = 1;

    loop {
        // 7. Create DPoP proof for upstream request
        let dpop_proof = server
            .token_manager
            .create_upstream_dpop_proof(
                method.as_str(),
                &upstream_url,
                Some(upstream_session_data.token_set.access_token.as_ref()),
                dpop_nonce.as_deref(),
                &upstream_dpop_key,
            )
            .await?;

        tracing::info!(
            "created DPoP proof for upstream request (retry {})",
            retry_count
        );

        // 8. Forward request to PDS
        let client = reqwest::Client::new();
        let mut request = client
            .request(method.clone(), &upstream_url)
            .header(
                "Authorization",
                format!("DPoP {}", upstream_session_data.token_set.access_token),
            )
            .header("DPoP", dpop_proof);

        // Copy relevant headers (skip auth/dpop/host)
        for (name, value) in headers.iter() {
            if !["host", "authorization", "dpop"].contains(&name.as_str()) {
                request = request.header(name, value);
            }
        }

        if !body.is_empty() {
            request = request.body(body.clone());
        }

        // 9. Send request
        let response = request
            .send()
            .await
            .map_err(|e| Error::NetworkError(e.to_string()))?;

        tracing::info!("upstream response status: {}", response.status());

        // 10. Check for DPoP nonce requirement (can be 400 or 401)
        if response.status() == 400 || response.status() == 401 {
            tracing::info!(
                "got {} error, checking for DPoP-Nonce header",
                response.status()
            );
            // Check if this is a use_dpop_nonce error
            if let Some(new_nonce) = response.headers().get("DPoP-Nonce") {
                if let Ok(nonce_str) = new_nonce.to_str() {
                    tracing::info!("found DPoP-Nonce header: {}", nonce_str);
                    if retry_count < max_retries {
                        // Store the new nonce and retry
                        dpop_nonce = Some(nonce_str.to_string());
                        server
                            .session_store
                            .update_session_dpop_nonce(&session_id, nonce_str.to_string())
                            .await?;
                        tracing::info!("received DPoP nonce, retrying with nonce: {}", nonce_str);
                        retry_count += 1;
                        continue;
                    }
                }
            } else {
                tracing::info!(
                    "no DPoP-Nonce header found in {} response",
                    response.status()
                );
            }
        }

        // 11. Handle successful DPoP nonce updates
        if let Some(new_nonce) = response.headers().get("DPoP-Nonce") {
            if let Ok(nonce_str) = new_nonce.to_str() {
                // Store new nonce for next request
                let _ = server
                    .session_store
                    .update_session_dpop_nonce(&session_id, nonce_str.to_string())
                    .await;
                tracing::info!("updated DPoP nonce from response");
            }
        }

        // 12. Return response
        let status = response.status();
        let resp_headers = response.headers().clone();
        let body = response
            .bytes()
            .await
            .map_err(|e| Error::NetworkError(e.to_string()))?;

        tracing::info!(
            "returning response to client: status={}, body_len={}, headers={:?}",
            status,
            body.len(),
            resp_headers
        );

        let mut response_builder = axum::http::Response::builder().status(status);
        for (name, value) in resp_headers.iter() {
            // Skip transfer-encoding since we've already consumed the body
            if name == "transfer-encoding" {
                continue;
            }
            response_builder = response_builder.header(name, value);
        }

        return Ok(response_builder
            .body(body.into())
            .map_err(|e| Error::InvalidRequest(e.to_string()))?);
    }
}

// Builder for OAuthProxyServer.
pub struct OAuthProxyServerBuilder<S, K>
where
    S: OAuthSessionStore + ClientAuthStore + Clone,
    K: KeyStore + Clone,
{
    config: Option<ProxyConfig>,
    session_store: Option<Arc<S>>,
    key_store: Option<Arc<K>>,
}

impl<S, K> Default for OAuthProxyServerBuilder<S, K>
where
    S: OAuthSessionStore + ClientAuthStore + Clone,
    K: KeyStore + Clone,
{
    fn default() -> Self {
        Self {
            config: None,
            session_store: None,
            key_store: None,
        }
    }
}

impl<S, K> OAuthProxyServerBuilder<S, K>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
{
    pub fn config(mut self, config: ProxyConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn session_store(mut self, store: Arc<S>) -> Self {
        self.session_store = Some(store);
        self
    }

    pub fn key_store(mut self, store: Arc<K>) -> Self {
        self.key_store = Some(store);
        self
    }

    pub fn build(self) -> Result<OAuthProxyServer<S, K>> {
        let config = self
            .config
            .ok_or_else(|| Error::InvalidRequest("config required".to_string()))?;
        let session_store = self
            .session_store
            .ok_or_else(|| Error::InvalidRequest("session_store required".to_string()))?;
        let key_store = self
            .key_store
            .ok_or_else(|| Error::InvalidRequest("key_store required".to_string()))?;

        let token_manager = Arc::new(TokenManager::new(config.host.to_string()));

        // Get the signing key for client authentication
        let signing_key = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(key_store.get_signing_key())
        })?;

        // Convert p256 signing key to jose_jwk::Jwk format
        let verifying_key = signing_key.verifying_key();
        let encoded_point = verifying_key.to_encoded_point(false);
        let x = encoded_point
            .x()
            .ok_or_else(|| Error::InvalidRequest("missing x coordinate".to_string()))?;
        let y = encoded_point
            .y()
            .ok_or_else(|| Error::InvalidRequest("missing y coordinate".to_string()))?;

        // Get the private key (d parameter)
        let d_bytes = signing_key.to_bytes();

        let jwk = jose_jwk::Jwk {
            key: jose_jwk::Key::Ec(jose_jwk::Ec {
                crv: jose_jwk::EcCurves::P256,
                x: jose_jwk::jose_b64::serde::Bytes::from(x.iter().as_slice().to_vec()),
                y: jose_jwk::jose_b64::serde::Bytes::from(y.iter().as_slice().to_vec()),
                d: Some(jose_jwk::jose_b64::serde::Secret::from(
                    d_bytes.iter().as_slice().to_vec(),
                )),
            }),
            prm: jose_jwk::Parameters {
                kid: Some("proxy-signing-key".into()),
                ..Default::default()
            },
        };

        // Create keyset with our signing key
        let keyset = jacquard_oauth::keyset::Keyset::try_from(vec![jwk])
            .map_err(|e| Error::InvalidRequest(format!("failed to create keyset: {}", e)))?;

        // Create OAuth client for upstream PDS authentication
        let client_data = ClientData {
            keyset: Some(keyset),
            config: config.client_metadata.clone(),
        };
        let oauth_client = Arc::new(OAuthClient::new((*session_store).clone(), client_data));

        Ok(OAuthProxyServer {
            config,
            session_store,
            key_store,
            token_manager,
            oauth_client,
        })
    }
}

// Request/response types

#[derive(Debug, Deserialize)]
struct PARRequest {
    client_id: String,
    redirect_uri: String,
    response_type: String,
    state: Option<String>,
    scope: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    login_hint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthorizeParams {
    client_id: Option<String>,
    redirect_uri: Option<String>,
    response_type: Option<String>,
    state: Option<String>,
    scope: Option<String>,
    request_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    iss: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenRequest {
    grant_type: String,
    code: Option<String>,
    refresh_token: Option<String>,
    client_id: Option<String>,
    redirect_uri: Option<String>,
}

#[derive(Debug, Serialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: Option<String>,
    scope: String,
    sub: String,
}

// Helper functions

fn extract_dpop_jkt_and_key(headers: &HeaderMap) -> Result<(String, jose_jwk::Jwk)> {
    use base64::prelude::*;

    // Get the DPoP header
    let dpop_proof = headers
        .get("DPoP")
        .and_then(|v| v.to_str().ok())
        .ok_or(Error::DpopProofRequired)?;

    // DPoP proof is a JWT - parse the header to get the JWK
    // JWT format: header.payload.signature
    let parts: Vec<&str> = dpop_proof.split('.').collect();
    if parts.len() != 3 {
        return Err(Error::InvalidRequest(
            "invalid DPoP proof format".to_string(),
        ));
    }

    // Decode the header (first part)
    let header_json = BASE64_URL_SAFE_NO_PAD
        .decode(parts[0])
        .map_err(|e| Error::InvalidRequest(format!("invalid DPoP header encoding: {}", e)))?;

    let header: serde_json::Value = serde_json::from_slice(&header_json)
        .map_err(|e| Error::InvalidRequest(format!("invalid DPoP header JSON: {}", e)))?;

    // Extract the JWK from the header
    let jwk_value = header
        .get("jwk")
        .ok_or_else(|| Error::InvalidRequest("DPoP proof missing jwk in header".to_string()))?;

    // Parse JWK
    let jwk: jose_jwk::Jwk = serde_json::from_value(jwk_value.clone())
        .map_err(|e| Error::InvalidRequest(format!("invalid JWK: {}", e)))?;

    // Compute the JWK thumbprint (JKT) according to RFC 7638
    let jkt = compute_jwk_thumbprint_from_json(jwk_value)?;

    Ok((jkt, jwk))
}

fn extract_dpop_jkt(headers: &HeaderMap) -> Result<String> {
    extract_dpop_jkt_and_key(headers).map(|(jkt, _)| jkt)
}

fn compute_jwk_thumbprint(jwk: &jose_jwk::Jwk) -> Result<String> {
    let jwk_value = serde_json::to_value(jwk)
        .map_err(|e| Error::InvalidRequest(format!("failed to serialize JWK: {}", e)))?;
    compute_jwk_thumbprint_from_json(&jwk_value)
}

fn compute_jwk_thumbprint_from_json(jwk: &serde_json::Value) -> Result<String> {
    use base64::prelude::*;
    use sha2::{Digest, Sha256};

    // Get the key type
    let kty = jwk
        .get("kty")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::InvalidRequest("JWK missing kty field".to_string()))?;

    // Create canonical JSON representation according to RFC 7638
    // Different key types require different fields, in lexicographic order
    let canonical = match kty {
        "EC" => {
            // EC key: requires crv, kty, x, y (in lexicographic order)
            let crv = jwk
                .get("crv")
                .ok_or_else(|| Error::InvalidRequest("EC JWK missing crv".to_string()))?;
            let x = jwk
                .get("x")
                .ok_or_else(|| Error::InvalidRequest("EC JWK missing x".to_string()))?;
            let y = jwk
                .get("y")
                .ok_or_else(|| Error::InvalidRequest("EC JWK missing y".to_string()))?;

            serde_json::json!({
                "crv": crv,
                "kty": kty,
                "x": x,
                "y": y,
            })
        }
        "RSA" => {
            // RSA key: requires e, kty, n (in lexicographic order)
            let e = jwk
                .get("e")
                .ok_or_else(|| Error::InvalidRequest("RSA JWK missing e".to_string()))?;
            let n = jwk
                .get("n")
                .ok_or_else(|| Error::InvalidRequest("RSA JWK missing n".to_string()))?;

            serde_json::json!({
                "e": e,
                "kty": kty,
                "n": n,
            })
        }
        "OKP" => {
            // OKP key: requires crv, kty, x (in lexicographic order)
            let crv = jwk
                .get("crv")
                .ok_or_else(|| Error::InvalidRequest("OKP JWK missing crv".to_string()))?;
            let x = jwk
                .get("x")
                .ok_or_else(|| Error::InvalidRequest("OKP JWK missing x".to_string()))?;

            serde_json::json!({
                "crv": crv,
                "kty": kty,
                "x": x,
            })
        }
        _ => {
            return Err(Error::InvalidRequest(format!(
                "unsupported JWK key type: {}",
                kty
            )));
        }
    };

    // Serialize to JSON without whitespace
    let canonical_json = serde_json::to_string(&canonical)
        .map_err(|e| Error::InvalidRequest(format!("failed to serialize JWK: {}", e)))?;

    // Compute SHA-256 hash
    let mut hasher = Sha256::new();
    hasher.update(canonical_json.as_bytes());
    let hash = hasher.finalize();

    // Encode as base64url
    Ok(BASE64_URL_SAFE_NO_PAD.encode(&hash))
}

fn generate_random_string(len: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

// Simple ReplayStore implementation that wraps our OAuthSessionStore
struct SimpleReplayStore<S: OAuthSessionStore> {
    session_store: Arc<S>,
}

impl<S: OAuthSessionStore> SimpleReplayStore<S> {
    fn new(session_store: Arc<S>) -> Self {
        Self { session_store }
    }
}

#[async_trait::async_trait]
impl<S: OAuthSessionStore + Send + Sync> dpop_verifier::ReplayStore for SimpleReplayStore<S> {
    async fn insert_once(
        &mut self,
        jti_hash: [u8; 32],
        _ctx: dpop_verifier::ReplayContext<'_>,
    ) -> std::result::Result<bool, dpop_verifier::DpopError> {
        // Convert jti_hash to hex string for storage
        let jti_str = hex::encode(jti_hash);

        // Check if this JTI has been used before
        let is_new = self
            .session_store
            .check_and_consume_nonce(&jti_str)
            .await
            .map_err(|_| dpop_verifier::DpopError::Replay)?;

        Ok(is_new)
    }
}
