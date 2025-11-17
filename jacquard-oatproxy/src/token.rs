use crate::error::Result;
use crate::session::OAuthSession;
use crate::store::{KeyStore, OAuthSessionStore};
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

    /// Validate a downstream JWT and extract claims
    pub async fn validate_downstream_jwt(
        &self,
        jwt: &str,
        key_store: &impl KeyStore,
    ) -> Result<DownstreamTokenClaims> {
        use base64::Engine;
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use p256::ecdsa::signature::Verifier;

        // Parse JWT (header.payload.signature)
        let parts: Vec<&str> = jwt.split('.').collect();
        if parts.len() != 3 {
            return Err(crate::error::Error::InvalidRequest(
                "invalid JWT format".to_string(),
            ));
        }

        // Decode header to verify it's the right algorithm
        let header_json = URL_SAFE_NO_PAD.decode(parts[0]).map_err(|e| {
            crate::error::Error::InvalidRequest(format!("invalid header encoding: {}", e))
        })?;

        let header: serde_json::Value = serde_json::from_slice(&header_json).map_err(|e| {
            crate::error::Error::InvalidRequest(format!("invalid header JSON: {}", e))
        })?;

        // Verify algorithm
        let alg = header.get("alg").and_then(|v| v.as_str()).ok_or_else(|| {
            crate::error::Error::InvalidRequest("missing alg in header".to_string())
        })?;

        if alg != "ES256" {
            return Err(crate::error::Error::InvalidRequest(format!(
                "unsupported algorithm: {}",
                alg
            )));
        }

        // Decode payload
        let payload_json = URL_SAFE_NO_PAD.decode(parts[1]).map_err(|e| {
            crate::error::Error::InvalidRequest(format!("invalid payload encoding: {}", e))
        })?;

        // Decode signature
        let signature_bytes = URL_SAFE_NO_PAD.decode(parts[2]).map_err(|e| {
            crate::error::Error::InvalidRequest(format!("invalid signature encoding: {}", e))
        })?;

        // Get signing key for validation
        let signing_key = key_store.get_signing_key().await?;
        let verifying_key = signing_key.verifying_key();

        // Verify signature
        let signature_input = format!("{}.{}", parts[0], parts[1]);
        let signature = p256::ecdsa::Signature::from_bytes(signature_bytes.as_slice().into())
            .map_err(|e| {
                crate::error::Error::InvalidRequest(format!("invalid signature format: {}", e))
            })?;

        verifying_key
            .verify(signature_input.as_bytes(), &signature)
            .map_err(|_| {
                crate::error::Error::InvalidRequest("signature verification failed".to_string())
            })?;

        // Parse claims
        let claims: DownstreamTokenClaims = serde_json::from_slice(&payload_json)
            .map_err(|e| crate::error::Error::InvalidRequest(format!("invalid claims: {}", e)))?;

        // Verify issuer
        if claims.iss != self.issuer {
            return Err(crate::error::Error::InvalidRequest(
                "wrong issuer".to_string(),
            ));
        }

        // Check expiry
        let now = Utc::now().timestamp();
        if claims.exp < now {
            return Err(crate::error::Error::InvalidRequest(
                "token expired".to_string(),
            ));
        }

        Ok(claims)
    }

    /// Refresh upstream tokens if they're about to expire
    pub async fn refresh_upstream_if_needed<S, K>(
        &self,
        session: &mut OAuthSession,
        session_store: &S,
        key_store: &K,
    ) -> Result<()>
    where
        S: OAuthSessionStore,
        K: KeyStore,
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
        key: &jose_jwk::Jwk,
        method: Method,
        url: &Url,
        nonce: Option<&str>,
    ) -> Result<String> {
        // Use the async implementation synchronously via blocking
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.create_upstream_dpop_proof(
                method.as_str(),
                url.as_str(),
                None, // no access token for token endpoint calls
                nonce,
                key,
            ))
        })
    }

    /// Create a DPoP proof for an upstream PDS request
    pub async fn create_upstream_dpop_proof(
        &self,
        method: &str,
        url: &str,
        access_token: Option<&str>,
        nonce: Option<&str>,
        dpop_jwk: &jose_jwk::Jwk,
    ) -> Result<String> {
        use base64::Engine;
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use jacquard_oauth::jose::{
            create_signed_jwt,
            jws::RegisteredHeader,
            jwt::{Claims, PublicClaims, RegisteredClaims},
        };
        use jose_jwk::jose_jwa::{Algorithm, Signing};
        use sha2::{Digest, Sha256};

        let now = Utc::now().timestamp();
        let jti = generate_random_string(32);

        // Hash access token if provided (for token-bound requests)
        let ath = if let Some(token) = access_token {
            let mut hasher = Sha256::new();
            hasher.update(token.as_bytes());
            let hash = hasher.finalize();
            Some(URL_SAFE_NO_PAD.encode(&hash).into())
        } else {
            None
        };

        // Create DPoP JWT claims
        let registered = RegisteredClaims {
            iss: None,
            sub: None,
            aud: None,
            exp: None,
            nbf: None,
            iat: Some(now),
            jti: Some(jti.into()),
        };

        let public = PublicClaims {
            htm: Some(method.into()),
            htu: Some(url.into()),
            ath,
            nonce: nonce.map(|n| n.into()),
        };

        let claims = Claims { registered, public };

        // Create header with JWK included (public key only)
        let mut header = RegisteredHeader::from(Algorithm::Signing(Signing::Es256));
        header.typ = Some("dpop+jwt".into());

        // Create a public-only version of the JWK for the header
        let public_jwk = jose_jwk::Jwk {
            key: match &dpop_jwk.key {
                jose_jwk::Key::Ec(ec) => {
                    // Keep only public parameters (crv, x, y), strip private key (d)
                    jose_jwk::Key::Ec(jose_jwk::Ec {
                        crv: ec.crv.clone(),
                        x: ec.x.clone(),
                        y: ec.y.clone(),
                        d: None, // Remove private key
                    })
                }
                _ => dpop_jwk.key.clone(),
            },
            prm: dpop_jwk.prm.clone(),
        };
        header.jwk = Some(public_jwk);

        // Extract the secret key from the JWK for signing
        let signing_key = match jose_jwk::crypto::Key::try_from(&dpop_jwk.key)
            .map_err(|e| crate::error::Error::InvalidRequest(format!("invalid key: {:?}", e)))?
        {
            jose_jwk::crypto::Key::P256(jose_jwk::crypto::Kind::Secret(secret)) => secret,
            _ => {
                return Err(crate::error::Error::InvalidRequest(
                    "DPoP key must be P256 secret key".to_string(),
                ));
            }
        };

        // Use jacquard-oauth's create_signed_jwt
        let dpop_proof =
            create_signed_jwt(signing_key.into(), header.into(), claims).map_err(|e| {
                crate::error::Error::InvalidRequest(format!("failed to sign DPoP proof: {}", e))
            })?;

        Ok(dpop_proof.to_string())
    }
}

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

/// Claims from a downstream JWT issued by the proxy
#[derive(Debug, serde::Deserialize)]
pub struct DownstreamTokenClaims {
    pub iss: String,
    pub sub: String, // account DID
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub scope: String,
    pub cnf: ConfirmationClaim,
}

/// DPoP confirmation claim
#[derive(Debug, serde::Deserialize)]
pub struct ConfirmationClaim {
    pub jkt: String, // DPoP JKT
}

const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
fn generate_random_string(len: usize) -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}
