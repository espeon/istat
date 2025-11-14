use crate::{
    config::ProxyConfig,
    error::{Error, Result},
    store::{KeyStore, NonceStore, OAuthSessionStore},
    token::TokenManager,
};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
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
pub struct OAuthProxyServer<S, K, N>
where
    S: OAuthSessionStore + ClientAuthStore + Clone,
    K: KeyStore + Clone,
    N: NonceStore + Clone,
{
    config: ProxyConfig,
    session_store: Arc<S>,
    key_store: Arc<K>,
    nonce_store: Arc<N>,
    token_manager: Arc<TokenManager>,
    oauth_client: Arc<OAuthClient<JacquardResolver, S>>,
}

impl<S, K, N> OAuthProxyServer<S, K, N>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
    N: NonceStore + Clone + 'static,
{
    /// Create a new OAuth proxy server builder.
    pub fn builder() -> OAuthProxyServerBuilder<S, K, N> {
        OAuthProxyServerBuilder::default()
    }

    /// Create the axum router with all OAuth endpoints.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/oauth/par", post(handle_par))
            .route("/oauth/authorize", get(handle_authorize))
            .route("/oauth/return", get(handle_return))
            .route("/oauth/token", post(handle_token))
            .route("/oauth/revoke", post(handle_revoke))
            .fallback(handle_xrpc_proxy)
            .with_state(self.clone())
    }
}

// OAuth handler functions

/// Handle Pushed Authorization Request (PAR).
async fn handle_par<S, K, N>(
    State(server): State<OAuthProxyServer<S, K, N>>,
    _headers: HeaderMap,
    body: String,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
    N: NonceStore + Clone + 'static,
{
    tracing::info!("handling PAR request");

    // Parse the PAR parameters
    let params: PARRequest =
        serde_urlencoded::from_str(&body).map_err(|e| Error::InvalidRequest(e.to_string()))?;

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
        expires_at: chrono::Utc::now() + chrono::Duration::seconds(90),
    };

    server
        .session_store
        .store_par_data(&request_uri, par_data)
        .await?;

    tracing::info!("stored PAR data with request_uri: {}", request_uri);

    let response = serde_json::json!({
        "request_uri": request_uri,
        "expires_in": 90
    });

    Ok(Json(response).into_response())
}

/// Handle authorization request - redirect to upstream PDS.
async fn handle_authorize<S, K, N>(
    State(server): State<OAuthProxyServer<S, K, N>>,
    Query(params): Query<AuthorizeParams>,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
    N: NonceStore + Clone + 'static,
{
    tracing::info!("handling authorize request");

    // If request_uri is provided, retrieve PAR data
    let (client_id, redirect_uri, response_type, state, scope) = if let Some(ref request_uri) =
        params.request_uri
    {
        tracing::info!("using PAR request_uri: {}", request_uri);

        let par_data = server
            .session_store
            .consume_par_data(request_uri)
            .await?
            .ok_or_else(|| Error::InvalidRequest("invalid or expired request_uri".to_string()))?;

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
        )
    };

    tracing::info!("handling authorize request for client_id: {}", client_id);

    // Store downstream client info so we can redirect back after upstream auth
    let downstream_info = crate::store::DownstreamClientInfo {
        redirect_uri: redirect_uri.clone(),
        state: state.clone(),
        response_type: response_type.clone(),
        scope: scope.clone(),
        expires_at: chrono::Utc::now() + chrono::Duration::minutes(10),
    };

    server
        .session_store
        .store_downstream_client_info(&client_id, downstream_info)
        .await?;

    // Use jacquard OAuth client to start upstream auth flow
    // This will resolve the PDS, create PAR, and return the authorization URL
    let auth_options = jacquard_oauth::types::AuthorizeOptions {
        scopes: server.config.scope.clone(),
        ..Default::default()
    };

    let auth_url = server
        .oauth_client
        .start_auth(&client_id, auth_options)
        .await
        .map_err(|e| Error::InvalidRequest(format!("failed to start auth: {}", e)))?;

    tracing::info!("redirecting to upstream PDS auth: {}", auth_url);
    Ok(Redirect::to(&auth_url).into_response())
}

/// Handle OAuth callback from upstream PDS.
async fn handle_return<S, K, N>(
    State(server): State<OAuthProxyServer<S, K, N>>,
    Query(params): Query<CallbackParams>,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
    N: NonceStore + Clone + 'static,
{
    tracing::info!("handling OAuth callback");

    // Check for errors from upstream PDS
    if let Some(error) = params.error {
        tracing::error!("upstream auth error: {}", error);
        return Err(Error::InvalidRequest(format!(
            "upstream auth failed: {}",
            error
        )));
    }

    let code = params.code.as_deref().ok_or_else(|| Error::InvalidGrant)?;
    let state = params
        .state
        .as_deref()
        .ok_or_else(|| Error::InvalidRequest("missing state".to_string()))?;

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
        .map_err(|e| Error::InvalidRequest(format!("failed to exchange code: {}", e)))?;

    // Extract session data
    let session_data = oauth_session.data.read().await;
    let account_did = session_data.account_did.to_string();
    let _pds_url = session_data.host_url.to_string();
    let upstream_session_id = session_data.session_id.to_string();

    tracing::info!(
        "successfully exchanged code for upstream tokens, DID: {}",
        account_did
    );
    drop(session_data); // release the read lock

    // Retrieve downstream client info that we stored in the authorize handler
    let downstream_client_info = server
        .session_store
        .consume_downstream_client_info(&account_did)
        .await?
        .ok_or_else(|| {
            Error::InvalidRequest(
                "downstream client info not found - authorization may have expired".to_string(),
            )
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
    let redirect_url = format!(
        "{}?code={}&state={}",
        pending_auth.redirect_uri,
        urlencoding::encode(&downstream_code),
        urlencoding::encode(&pending_auth.state.unwrap_or_default())
    );

    Ok(Redirect::to(&redirect_url).into_response())
}

/// Handle token request (exchange code for tokens or refresh tokens).
async fn handle_token<S, K, N>(
    State(server): State<OAuthProxyServer<S, K, N>>,
    headers: HeaderMap,
    body: String,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
    N: NonceStore + Clone + 'static,
{
    tracing::info!("handling token request");

    // Parse token request
    let params: TokenRequest =
        serde_urlencoded::from_str(&body).map_err(|e| Error::InvalidRequest(e.to_string()))?;

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

            // Issue downstream JWT bound to client's DPoP key
            let access_token = server
                .token_manager
                .issue_downstream_jwt(
                    &pending_auth.account_did,
                    &dpop_jkt,
                    &scope_str,
                    3600, // 1 hour expiry
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

            let response = TokenResponse {
                access_token,
                token_type: "DPoP".to_string(),
                expires_in: 3600,
                refresh_token: Some(downstream_refresh_token),
                scope: scope_str,
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
                    3600,
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

            let response = TokenResponse {
                access_token,
                token_type: "DPoP".to_string(),
                expires_in: 3600,
                refresh_token: Some(new_downstream_refresh),
                scope: scope_str,
            };

            Ok(Json(response).into_response())
        }
        _ => Err(Error::InvalidGrant),
    }
}

/// Handle token revocation.
async fn handle_revoke<S, K, N>(
    State(server): State<OAuthProxyServer<S, K, N>>,
    headers: HeaderMap,
    _body: String,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone,
    K: KeyStore + Clone,
    N: NonceStore + Clone,
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
async fn handle_xrpc_proxy<S, K, N>(
    State(server): State<OAuthProxyServer<S, K, N>>,
    method: Method,
    headers: HeaderMap,
    body: String,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone,
    K: KeyStore + Clone,
    N: NonceStore + Clone,
{
    tracing::info!(
        "proxying XRPC request: {} {}",
        method,
        headers
            .get("uri")
            .map(|v| v.to_str().unwrap_or(""))
            .unwrap_or("")
    );

    // Extract DPoP JKT and look up session
    let dpop_jkt = extract_dpop_jkt(&headers)?;
    let mut session = server
        .session_store
        .get_by_dpop_jkt(&dpop_jkt)
        .await?
        .ok_or(Error::SessionNotFound)?;

    // Check if upstream token needs refresh
    server
        .token_manager
        .refresh_upstream_if_needed(
            &mut session,
            &*server.session_store,
            &*server.key_store,
            &*server.nonce_store,
        )
        .await?;

    // Build the upstream request
    let uri = headers
        .get("uri")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| Error::InvalidRequest("missing uri".to_string()))?;

    let upstream_url = format!("{}{}", session.pds_url, uri);

    // TODO: Create proper DPoP proof for upstream request
    // For now, forward without DPoP

    // Forward the request
    let client = reqwest::Client::new();
    let mut request = client.request(method.clone(), &upstream_url).header(
        "Authorization",
        format!("DPoP {}", session.upstream_access_token),
    );

    // Copy relevant headers
    for (name, value) in headers.iter() {
        if !["host", "authorization", "dpop"].contains(&name.as_str()) {
            request = request.header(name, value);
        }
    }

    if !body.is_empty() {
        request = request.body(body);
    }

    let response = request
        .send()
        .await
        .map_err(|e| Error::NetworkError(e.to_string()))?;

    // Build response
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .text()
        .await
        .map_err(|e| Error::NetworkError(e.to_string()))?;

    let mut response_builder = axum::http::Response::builder().status(status);
    for (name, value) in headers.iter() {
        response_builder = response_builder.header(name, value);
    }

    Ok(response_builder
        .body(body.into())
        .map_err(|e| Error::InvalidRequest(e.to_string()))?)
}

// Builder for OAuthProxyServer.
pub struct OAuthProxyServerBuilder<S, K, N>
where
    S: OAuthSessionStore + ClientAuthStore + Clone,
    K: KeyStore + Clone,
    N: NonceStore + Clone,
{
    config: Option<ProxyConfig>,
    session_store: Option<Arc<S>>,
    key_store: Option<Arc<K>>,
    nonce_store: Option<Arc<N>>,
}

impl<S, K, N> Default for OAuthProxyServerBuilder<S, K, N>
where
    S: OAuthSessionStore + ClientAuthStore + Clone,
    K: KeyStore + Clone,
    N: NonceStore + Clone,
{
    fn default() -> Self {
        Self {
            config: None,
            session_store: None,
            key_store: None,
            nonce_store: None,
        }
    }
}

impl<S, K, N> OAuthProxyServerBuilder<S, K, N>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
    N: NonceStore + Clone + 'static,
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

    pub fn nonce_store(mut self, store: Arc<N>) -> Self {
        self.nonce_store = Some(store);
        self
    }

    pub fn build(self) -> Result<OAuthProxyServer<S, K, N>> {
        let config = self
            .config
            .ok_or_else(|| Error::InvalidRequest("config required".to_string()))?;
        let session_store = self
            .session_store
            .ok_or_else(|| Error::InvalidRequest("session_store required".to_string()))?;
        let key_store = self
            .key_store
            .ok_or_else(|| Error::InvalidRequest("key_store required".to_string()))?;
        let nonce_store = self
            .nonce_store
            .ok_or_else(|| Error::InvalidRequest("nonce_store required".to_string()))?;

        let token_manager = Arc::new(TokenManager::new(config.host.to_string()));

        // Create OAuth client for upstream PDS authentication
        let client_data = ClientData {
            keyset: None,
            config: config.client_metadata.clone(),
        };
        let oauth_client = Arc::new(OAuthClient::new((*session_store).clone(), client_data));

        Ok(OAuthProxyServer {
            config,
            session_store,
            key_store,
            nonce_store,
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
}

// Helper functions

fn extract_dpop_jkt(headers: &HeaderMap) -> Result<String> {
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

    // Compute the JWK thumbprint (JKT) according to RFC 7638
    let jkt = compute_jwk_thumbprint_from_json(jwk_value)?;

    Ok(jkt)
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
