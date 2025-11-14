use crate::error::Result;
use crate::session::OAuthSession;
use crate::store::{KeyStore, NonceStore, OAuthSessionStore};
use chrono::{Duration, Utc};
use http::Method;
use serde_json::json;
use url::Url;

/// Manages token issuance and refresh
pub struct TokenManager {
    // For issuing downstream JWTs
    issuer: String,
}

impl TokenManager {
    pub fn new(issuer: String) -> Self {
        Self { issuer }
    }

    /// Issue a downstream JWT access token for the client
    pub async fn issue_downstream_jwt(
        &self,
        sub: &str,
        dpop_jkt: &str,
        scope: &str,
        expires_in_seconds: i64,
        key_store: &impl KeyStore,
    ) -> Result<String> {
        use jacquard_oauth::jose::jws::RegisteredHeader;
        use jose_jwk::jose_jwa::{Algorithm, Signing};

        let signing_key = key_store.get_signing_key().await?;

        let now = Utc::now().timestamp();
        let exp = now + expires_in_seconds;

        // Create claims JSON with custom fields
        let claims_json = json!({
            "iss": self.issuer,
            "sub": sub,
            "aud": self.issuer,
            "exp": exp,
            "iat": now,
            "scope": scope,
            "cnf": {
                "jkt": dpop_jkt,
            },
        });

        let claims_str = serde_json::to_string(&claims_json).map_err(|e| {
            crate::error::Error::InvalidRequest(format!("failed to serialize claims: {}", e))
        })?;

        // Create JWS header
        let mut header = RegisteredHeader::from(Algorithm::Signing(Signing::Es256));
        header.typ = Some("JWT".into());

        // Sign the JWT manually since we need custom claims
        use base64::Engine;
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use p256::ecdsa::signature::Signer;

        let header_json = serde_json::to_string(&header).map_err(|e| {
            crate::error::Error::InvalidRequest(format!("failed to serialize header: {}", e))
        })?;

        let header_b64 = URL_SAFE_NO_PAD.encode(&header_json);
        let payload_b64 = URL_SAFE_NO_PAD.encode(&claims_str);
        let signature_input = format!("{}.{}", header_b64, payload_b64);

        let signature: p256::ecdsa::Signature = signing_key.sign(signature_input.as_bytes());
        let signature_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());

        let jwt = format!("{}.{}.{}", header_b64, payload_b64, signature_b64);

        Ok(jwt)
    }

    /// Issue a downstream JWT access token for the client (legacy method for OAuthSession)
    pub async fn issue_downstream_access_token(
        &self,
        session: &OAuthSession,
        key_store: &impl KeyStore,
    ) -> Result<String> {
        self.issue_downstream_jwt(
            session.did.as_str(),
            &session.downstream_dpop_key_thumbprint,
            &session.upstream_scope,
            24 * 3600, // 24 hours
            key_store,
        )
        .await
    }

    /// Refresh upstream tokens if they're about to expire
    pub async fn refresh_upstream_if_needed<S, K, N>(
        &self,
        session: &mut OAuthSession,
        session_store: &S,
        key_store: &K,
        _nonce_store: &N,
    ) -> Result<()>
    where
        S: OAuthSessionStore,
        K: KeyStore,
        N: NonceStore,
    {
        // Check if refresh needed (5 min buffer)
        if !session.needs_refresh(5) {
            return Ok(());
        }

        // Get the DPoP key for upstream requests
        let dpop_key = key_store
            .get_dpop_key(&session.upstream_dpop_key_thumbprint)
            .await?
            .ok_or(crate::error::Error::KeyNotFound)?;

        // Create DPoP proof for token refresh
        let dpop_proof = self.create_dpop_proof(
            &dpop_key,
            Method::POST,
            &session.pds_url,
            session.upstream_dpop_nonce.as_deref(),
        )?;

        // Call PDS token endpoint with refresh grant
        let client = reqwest::Client::new();
        let token_url = format!("{}/oauth/token", session.pds_url);

        let response = client
            .post(&token_url)
            .header("DPoP", dpop_proof)
            .form(&[
                ("grant_type", "refresh_token"),
                (
                    "refresh_token",
                    session.upstream_refresh_token.as_ref().unwrap(),
                ),
            ])
            .send()
            .await
            .map_err(|e| crate::error::Error::NetworkError(e.to_string()))?;

        // Update nonce from response header
        if let Some(nonce) = response.headers().get("dpop-nonce") {
            session.upstream_dpop_nonce = Some(
                nonce
                    .to_str()
                    .map_err(|e| crate::error::Error::Internal(e.to_string()))?
                    .to_string(),
            );
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(|e| crate::error::Error::NetworkError(e.to_string()))?;

        // Update session with new tokens
        session.upstream_access_token = token_response.access_token;
        if let Some(refresh) = token_response.refresh_token {
            session.upstream_refresh_token = Some(refresh);
        }
        session.upstream_expires_at =
            Utc::now() + Duration::seconds(token_response.expires_in.unwrap_or(3600));
        session.last_used_at = Utc::now();

        // Persist updated session
        session_store.update_session(session).await?;

        Ok(())
    }

    fn create_dpop_proof(
        &self,
        _key: &jose_jwk::Key,
        method: Method,
        url: &Url,
        nonce: Option<&str>,
    ) -> Result<String> {
        let mut claims = json!({
            "jti": generate_jti(),
            "htm": method.as_str(),
            "htu": url.as_str(),
            "iat": Utc::now().timestamp(),
            "exp": (Utc::now() + Duration::minutes(1)).timestamp(),
        });

        if let Some(n) = nonce {
            claims["nonce"] = json!(n);
        }

        // TODO: Implement DPoP proof signing
        // For now, return a placeholder
        Ok(format!("dpop_proof_{}", claims["jti"]))
    }
}

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

fn generate_jti() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.r#gen();
    hex::encode(bytes)
}
