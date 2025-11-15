use chrono::{DateTime, Utc};
use jacquard_common::types::did::Did;
use serde::{Deserialize, Serialize};
use url::Url;

/// Unique identifier for an OAuth session
pub type SessionId = String;

/// State of an OAuth session through its lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// PAR created, awaiting authorization
    PendingPAR,
    /// User redirected to PDS, awaiting callback
    AwaitingAuthorization,
    /// Callback received, awaiting token exchange
    AwaitingTokenExchange,
    /// Fully authenticated and ready
    Ready,
    /// Session has been revoked
    Revoked,
}

/// OAuth session containing both upstream (proxy↔PDS) and downstream (client↔proxy) state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthSession {
    /// Unique session identifier
    pub id: SessionId,

    /// Current state of the session
    pub state: SessionState,

    // === User Identity ===
    /// User's DID
    #[serde(borrow)]
    pub did: Did<'static>,

    /// User's handle (optional)
    pub handle: Option<String>,

    /// User's PDS URL
    pub pds_url: Url,

    // === Upstream (proxy → PDS) ===
    /// Access token for PDS requests (long-lived, months)
    pub upstream_access_token: String,

    /// Refresh token for obtaining new access tokens
    pub upstream_refresh_token: Option<String>,

    /// Thumbprint of the DPoP key used for upstream requests
    pub upstream_dpop_key_thumbprint: String,

    /// When the upstream access token expires
    pub upstream_expires_at: DateTime<Utc>,

    /// Scope granted for upstream requests
    pub upstream_scope: String,

    /// Last nonce received from the PDS
    pub upstream_dpop_nonce: Option<String>,

    // === Downstream (client → proxy) ===
    /// Temporary authorization code for client token exchange
    pub downstream_auth_code: Option<String>,

    /// Refresh token issued to the client
    pub downstream_refresh_token: Option<String>,

    /// Thumbprint of the client's DPoP key (PRIMARY LOOKUP KEY)
    pub downstream_dpop_key_thumbprint: String,

    /// When the downstream access token expires (short-lived, hours/days)
    pub downstream_expires_at: DateTime<Utc>,

    /// Nonce pad for generating downstream nonces
    pub downstream_dpop_nonce_pad: String,

    // === DPoP Replay Protection ===
    /// Recent JTIs seen (for replay protection)
    pub jti_cache: Vec<String>,

    // === OAuth Flow State ===
    /// PAR request URI
    pub request_uri: Option<String>,

    /// PKCE code verifier
    pub pkce_verifier: Option<String>,

    /// Client's redirect URI
    pub downstream_redirect_uri: String,

    /// Client's client_id
    pub downstream_client_id: Option<String>,

    /// Client's state parameter
    pub downstream_state: Option<String>,

    // === Timestamps ===
    /// When this session was created
    pub created_at: DateTime<Utc>,

    /// When this session was last used
    pub last_used_at: DateTime<Utc>,
}

impl OAuthSession {
    /// Create a new session in pending state
    pub fn new(did: Did<'static>, pds_url: Url, client_id: String, redirect_uri: String) -> Self {
        Self {
            id: generate_session_id(),
            state: SessionState::AwaitingAuthorization,
            did,
            handle: None,
            pds_url,
            upstream_access_token: String::new(),
            upstream_refresh_token: None,
            upstream_dpop_key_thumbprint: String::new(),
            upstream_expires_at: Utc::now(),
            upstream_scope: String::new(),
            upstream_dpop_nonce: None,
            downstream_auth_code: None,
            downstream_refresh_token: None,
            downstream_dpop_key_thumbprint: String::new(),
            downstream_expires_at: Utc::now(),
            downstream_dpop_nonce_pad: generate_nonce_pad(),
            jti_cache: Vec::new(),
            request_uri: None,
            pkce_verifier: None,
            downstream_redirect_uri: redirect_uri,
            downstream_client_id: Some(client_id),
            downstream_state: None,
            created_at: Utc::now(),
            last_used_at: Utc::now(),
        }
    }

    /// Check if the session is ready for use
    pub fn is_ready(&self) -> bool {
        self.state == SessionState::Ready
    }

    /// Check if the session is revoked
    pub fn is_revoked(&self) -> bool {
        self.state == SessionState::Revoked
    }

    /// Check if the upstream token needs refresh
    pub fn needs_refresh(&self, buffer_minutes: i64) -> bool {
        self.upstream_expires_at < Utc::now() + chrono::Duration::minutes(buffer_minutes)
    }
}

fn generate_session_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.r#gen();
    hex::encode(bytes)
}

fn generate_nonce_pad() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.r#gen();
    hex::encode(bytes)
}
