//! Authentication helpers for downstream clients.
//!
//! Provides utilities for validating JWTs issued by the proxy and extracting
//! authenticated user information.

use crate::error::{Error, Result};
use crate::store::KeyStore;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// JWT claims issued by the proxy to downstream clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyJwtClaims {
    /// Subject (account DID)
    pub sub: String,
    /// Issuer (proxy URL)
    pub iss: String,
    /// Audience (client ID)
    pub aud: String,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Expiration time (Unix timestamp)
    pub exp: i64,
    /// Session ID
    pub session_id: String,
}

/// Validates a JWT issued by the proxy.
///
/// Returns the claims if valid, or an error if invalid/expired.
pub async fn validate_proxy_jwt<K: KeyStore>(
    token: &str,
    key_store: &K,
    expected_issuer: &str,
) -> Result<ProxyJwtClaims> {
    let signing_key = key_store.get_signing_key().await?;
    let verifying_key = signing_key.verifying_key();

    // Split JWT into parts
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(Error::InvalidRequest("invalid JWT format".to_string()));
    }

    let header_b64 = parts[0];
    let payload_b64 = parts[1];
    let signature_b64 = parts[2];

    // Decode signature
    let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(signature_b64)
        .map_err(|e| Error::InvalidRequest(format!("invalid signature: {}", e)))?;

    // Verify signature
    let message = format!("{}.{}", header_b64, payload_b64);
    let signature_bytes: [u8; 64] = signature
        .try_into()
        .map_err(|_| Error::InvalidRequest("invalid signature length".to_string()))?;

    let sig = p256::ecdsa::Signature::from_bytes(&signature_bytes.into())
        .map_err(|e| Error::InvalidRequest(format!("invalid signature: {}", e)))?;

    use p256::ecdsa::signature::Verifier;
    verifying_key
        .verify(message.as_bytes(), &sig)
        .map_err(|e| Error::InvalidRequest(format!("JWT verification failed: {}", e)))?;

    // Decode payload
    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|e| Error::InvalidRequest(format!("invalid payload: {}", e)))?;

    let claims: ProxyJwtClaims = serde_json::from_slice(&payload_bytes)
        .map_err(|e| Error::InvalidRequest(format!("invalid JWT claims: {}", e)))?;

    // Validate issuer
    if claims.iss != expected_issuer {
        return Err(Error::InvalidRequest(format!(
            "invalid issuer: expected {}, got {}",
            expected_issuer, claims.iss
        )));
    }

    // Validate expiration
    let now = chrono::Utc::now().timestamp();
    if claims.exp < now {
        return Err(Error::InvalidRequest("JWT expired".to_string()));
    }

    Ok(claims)
}

/// Extracts a bearer token from an Authorization header value.
///
/// Returns the token if present and valid, or None otherwise.
pub fn extract_bearer_token(auth_header: &str) -> Option<&str> {
    auth_header
        .strip_prefix("Bearer ")
        .or_else(|| auth_header.strip_prefix("bearer "))
}

#[cfg(feature = "axum")]
pub mod axum_extractors {
    //! Axum extractors for authenticated requests.

    use super::*;
    use axum::{
        extract::FromRequestParts,
        http::{StatusCode, request::Parts},
    };

    /// State required for JWT validation.
    #[derive(Clone)]
    pub struct AuthState<K: KeyStore> {
        pub key_store: Arc<K>,
        pub issuer: String,
    }

    impl<K: KeyStore> AuthState<K> {
        pub fn new(key_store: Arc<K>, issuer: String) -> Self {
            Self { key_store, issuer }
        }
    }

    /// Extractor for authenticated DID from JWT.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// async fn protected_handler(
    ///     AuthenticatedUser(did): AuthenticatedUser,
    /// ) -> String {
    ///     format!("Hello, {}!", did)
    /// }
    /// ```
    pub struct AuthenticatedUser<K: KeyStore>(pub String, std::marker::PhantomData<K>);

    impl<S, K> FromRequestParts<S> for AuthenticatedUser<K>
    where
        S: Send + Sync,
        K: KeyStore + Clone + Send + Sync + 'static,
        Arc<AuthState<K>>: axum::extract::FromRef<S>,
    {
        type Rejection = StatusCode;

        fn from_request_parts(
            parts: &mut Parts,
            state: &S,
        ) -> impl std::future::Future<Output = std::result::Result<Self, Self::Rejection>> + Send
        {
            let auth_state: Arc<AuthState<K>> =
                <Arc<AuthState<K>> as axum::extract::FromRef<S>>::from_ref(state);

            let auth_header = parts
                .headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|h| h.to_str().ok());

            async move {
                let token = extract_bearer_token(auth_header.ok_or(StatusCode::UNAUTHORIZED)?)
                    .ok_or(StatusCode::UNAUTHORIZED)?;

                let claims = validate_proxy_jwt(token, &*auth_state.key_store, &auth_state.issuer)
                    .await
                    .map_err(|_| StatusCode::UNAUTHORIZED)?;

                Ok(AuthenticatedUser(claims.sub, std::marker::PhantomData))
            }
        }
    }

    /// Extractor that provides full JWT claims.
    pub struct AuthenticatedClaims<K: KeyStore>(pub ProxyJwtClaims, std::marker::PhantomData<K>);

    impl<S, K> FromRequestParts<S> for AuthenticatedClaims<K>
    where
        S: Send + Sync,
        K: KeyStore + Clone + Send + Sync + 'static,
        Arc<AuthState<K>>: axum::extract::FromRef<S>,
    {
        type Rejection = StatusCode;

        fn from_request_parts(
            parts: &mut Parts,
            state: &S,
        ) -> impl std::future::Future<Output = std::result::Result<Self, Self::Rejection>> + Send
        {
            let auth_state: Arc<AuthState<K>> =
                <Arc<AuthState<K>> as axum::extract::FromRef<S>>::from_ref(state);

            let auth_header = parts
                .headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|h| h.to_str().ok());

            async move {
                let token = extract_bearer_token(auth_header.ok_or(StatusCode::UNAUTHORIZED)?)
                    .ok_or(StatusCode::UNAUTHORIZED)?;

                let claims = validate_proxy_jwt(token, &*auth_state.key_store, &auth_state.issuer)
                    .await
                    .map_err(|_| StatusCode::UNAUTHORIZED)?;

                Ok(AuthenticatedClaims(claims, std::marker::PhantomData))
            }
        }
    }
}
