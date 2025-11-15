use crate::error::Result;
use crate::session::{OAuthSession, SessionId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Information about a pending downstream authorization
#[derive(Debug, Clone)]
pub struct PendingAuth {
    /// Account DID from upstream auth
    pub account_did: String,
    /// Session ID from upstream (the state parameter)
    pub upstream_session_id: String,
    /// Downstream client's redirect URI
    pub redirect_uri: String,
    /// Downstream client's state parameter
    pub state: Option<String>,
    /// When this authorization expires
    pub expires_at: DateTime<Utc>,
}

/// Downstream client metadata for an authorization flow
#[derive(Debug, Clone)]
pub struct DownstreamClientInfo {
    /// Client's redirect URI
    pub redirect_uri: String,
    /// Client's state parameter
    pub state: Option<String>,
    /// Client's response type
    pub response_type: String,
    /// Requested scope
    pub scope: Option<String>,
    /// When this info expires
    pub expires_at: DateTime<Utc>,
}

/// PAR (Pushed Authorization Request) data
#[derive(Debug, Clone)]
pub struct PARData {
    /// Client ID
    pub client_id: String,
    /// Redirect URI
    pub redirect_uri: String,
    /// Response type
    pub response_type: String,
    /// State parameter
    pub state: Option<String>,
    /// Requested scope
    pub scope: Option<String>,
    /// PKCE code challenge
    pub code_challenge: Option<String>,
    /// PKCE code challenge method
    pub code_challenge_method: Option<String>,
    /// Login hint (user handle or DID)
    pub login_hint: Option<String>,
    /// Downstream client's DPoP JKT
    pub downstream_dpop_jkt: String,
    /// When this PAR expires (typically 90 seconds)
    pub expires_at: DateTime<Utc>,
}

/// Storage abstraction for OAuth sessions
#[async_trait]
pub trait OAuthSessionStore: Send + Sync {
    /// Create a new session
    async fn create_session(&self, session: OAuthSession) -> Result<SessionId>;

    /// Get a session by its ID
    async fn get_session(&self, id: &SessionId) -> Result<Option<OAuthSession>>;

    /// Update an existing session
    async fn update_session(&self, session: &OAuthSession) -> Result<()>;

    /// Delete a session
    async fn delete_session(&self, id: &SessionId) -> Result<()>;

    /// Get a session by PAR request URI
    async fn get_by_request_uri(&self, uri: &str) -> Result<Option<OAuthSession>>;

    /// Get a session by OAuth state parameter
    async fn get_by_state(&self, state: &str) -> Result<Option<OAuthSession>>;

    /// Get a session by downstream DPoP key thumbprint (PRIMARY LOOKUP)
    async fn get_by_dpop_jkt(&self, jkt: &str) -> Result<Option<OAuthSession>>;

    /// Store a pending authorization code mapping
    async fn store_pending_auth(&self, code: &str, auth: PendingAuth) -> Result<()>;

    /// Get and remove a pending authorization by code
    async fn consume_pending_auth(&self, code: &str) -> Result<Option<PendingAuth>>;

    /// Store downstream client info indexed by DID (user identifier)
    async fn store_downstream_client_info(
        &self,
        did: &str,
        info: DownstreamClientInfo,
    ) -> Result<()>;

    /// Get and remove downstream client info by DID
    async fn consume_downstream_client_info(
        &self,
        did: &str,
    ) -> Result<Option<DownstreamClientInfo>>;

    /// Store PAR data indexed by request_uri
    async fn store_par_data(&self, request_uri: &str, data: PARData) -> Result<()>;

    /// Get and remove PAR data by request_uri
    async fn consume_par_data(&self, request_uri: &str) -> Result<Option<PARData>>;

    /// Store refresh token mapping (refresh_token → account_did + session_id)
    async fn store_refresh_token_mapping(
        &self,
        refresh_token: &str,
        account_did: String,
        session_id: String,
    ) -> Result<()>;

    /// Get refresh token mapping by refresh token
    async fn get_refresh_token_mapping(
        &self,
        refresh_token: &str,
    ) -> Result<Option<(String, String)>>;

    /// Store active session mapping (DID → session_id)
    async fn store_active_session(&self, did: &str, session_id: String) -> Result<()>;

    /// Get active session for a DID
    async fn get_active_session(&self, did: &str) -> Result<Option<String>>;

    /// Store DPoP key for a session
    async fn store_session_dpop_key(
        &self,
        session_id: &str,
        dpop_jkt: String,
        key: jose_jwk::Jwk,
    ) -> Result<()>;

    /// Get DPoP key for a session
    async fn get_session_dpop_key(
        &self,
        session_id: &str,
    ) -> Result<Option<(String, jose_jwk::Jwk)>>;

    /// Store DPoP nonce for a session
    async fn update_session_dpop_nonce(&self, session_id: &str, nonce: String) -> Result<()>;

    /// Get DPoP nonce for a session
    async fn get_session_dpop_nonce(&self, session_id: &str) -> Result<Option<String>>;
}

/// Key management for OAuth tokens and DPoP proofs
#[async_trait]
pub trait KeyStore: Send + Sync {
    /// Get the proxy's JWT signing key for issuing downstream tokens
    /// Returns a P256 ECDSA signing key
    async fn get_signing_key(&self) -> Result<p256::ecdsa::SigningKey>;

    /// Create a new DPoP key for upstream PDS communication
    async fn create_dpop_key(&self) -> Result<jose_jwk::Jwk>;

    /// Get a DPoP key by its thumbprint
    async fn get_dpop_key(&self, thumbprint: &str) -> Result<Option<jose_jwk::Jwk>>;
}

/// Nonce management for DPoP replay protection
#[async_trait]
pub trait NonceStore: Send + Sync {
    /// Check if a nonce (JTI) is valid and consume it
    /// Returns true if the nonce was valid and hasn't been used
    async fn check_and_consume_nonce(&self, jti: &str) -> Result<bool>;

    /// Generate a new nonce value for response (nonce XOR nonce_pad)
    async fn generate_nonce(&self, session_id: &str, nonce_pad: &str) -> Result<String>;

    /// Store nonce pad for a session (used to generate and verify nonces)
    async fn store_nonce_pad(&self, session_id: &str, nonce_pad: &str) -> Result<()>;

    /// Get nonce pad for a session
    async fn get_nonce_pad(&self, session_id: &str) -> Result<Option<String>>;

    /// Verify that a nonce matches the expected format for this session
    /// (checks that nonce XOR nonce_pad produces valid result)
    async fn verify_nonce(&self, session_id: &str, nonce: &str) -> Result<bool>;

    /// Clean up expired nonces
    async fn cleanup_expired(&self, before: DateTime<Utc>) -> Result<()>;
}
