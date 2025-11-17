-- Initial schema for istat application

-- Emoji records
CREATE TABLE IF NOT EXISTS emojis (
    at TEXT PRIMARY KEY NOT NULL,
    did TEXT NOT NULL,
    blob_cid TEXT NOT NULL,
    mime_type TEXT,
    alt_text TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_emojis_did ON emojis(did);
CREATE INDEX IF NOT EXISTS idx_emojis_created_at ON emojis(created_at);

-- Status records
CREATE TABLE IF NOT EXISTS statuses (
    at TEXT PRIMARY KEY NOT NULL,
    did TEXT NOT NULL,
    rkey TEXT NOT NULL,
    emoji_ref TEXT NOT NULL,
    emoji_ref_cid TEXT NOT NULL,
    title TEXT,
    description TEXT,
    expires TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(did, rkey)
);

CREATE INDEX IF NOT EXISTS idx_statuses_did ON statuses(did);
CREATE INDEX IF NOT EXISTS idx_statuses_rkey ON statuses(rkey);
CREATE INDEX IF NOT EXISTS idx_statuses_created_at ON statuses(created_at);
CREATE INDEX IF NOT EXISTS idx_statuses_expires ON statuses(expires);

-- Auth sessions and tokens
CREATE TABLE IF NOT EXISTS auth_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    did TEXT NOT NULL,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    dpop_key_thumbprint TEXT NOT NULL,
    scope TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    last_used_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_did ON auth_sessions(did);
CREATE INDEX IF NOT EXISTS idx_auth_sessions_access_token ON auth_sessions(access_token);
CREATE INDEX IF NOT EXISTS idx_auth_sessions_expires_at ON auth_sessions(expires_at);

-- DPoP nonces for replay protection
CREATE TABLE IF NOT EXISTS dpop_nonces (
    jti TEXT PRIMARY KEY NOT NULL,
    used_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_dpop_nonces_used_at ON dpop_nonces(used_at);

-- OAuth Proxy Tables

-- Pending authorization codes
CREATE TABLE IF NOT EXISTS oatproxy_pending_auths (
    code TEXT PRIMARY KEY,
    account_did TEXT NOT NULL,
    upstream_session_id TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    state TEXT,
    expires_at TEXT NOT NULL
);

-- Downstream client info (temporary storage during OAuth flow)
CREATE TABLE IF NOT EXISTS oatproxy_downstream_clients (
    did TEXT PRIMARY KEY,
    redirect_uri TEXT NOT NULL,
    state TEXT,
    response_type TEXT NOT NULL,
    scope TEXT,
    expires_at TEXT NOT NULL
);

-- PAR (Pushed Authorization Request) data
CREATE TABLE IF NOT EXISTS oatproxy_par_data (
    request_uri TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    response_type TEXT NOT NULL,
    state TEXT,
    scope TEXT,
    code_challenge TEXT,
    code_challenge_method TEXT,
    login_hint TEXT,
    downstream_dpop_jkt TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

-- Refresh token mappings
CREATE TABLE IF NOT EXISTS oatproxy_refresh_tokens (
    refresh_token TEXT PRIMARY KEY,
    account_did TEXT NOT NULL,
    session_id TEXT NOT NULL
);

-- Active session mappings (DID → session_id)
CREATE TABLE IF NOT EXISTS oatproxy_active_sessions (
    did TEXT PRIMARY KEY,
    session_id TEXT NOT NULL
);

-- Session DPoP keys (upstream PDS communication keys)
CREATE TABLE IF NOT EXISTS oatproxy_session_dpop_keys (
    session_id TEXT PRIMARY KEY,
    dpop_jkt TEXT NOT NULL,
    key_json TEXT NOT NULL
);

-- Session DPoP nonces (for retry logic)
CREATE TABLE IF NOT EXISTS oatproxy_session_dpop_nonces (
    session_id TEXT PRIMARY KEY,
    nonce TEXT NOT NULL
);

-- Used nonces (JTI replay protection)
CREATE TABLE IF NOT EXISTS oatproxy_used_nonces (
    jti TEXT PRIMARY KEY,
    created_at TEXT NOT NULL
);

-- OAuth sessions (jacquard-oauth ClientAuthStore data)
CREATE TABLE IF NOT EXISTS oatproxy_oauth_sessions (
    did TEXT NOT NULL,
    session_id TEXT NOT NULL,
    session_data TEXT NOT NULL,
    PRIMARY KEY (did, session_id)
);

-- OAuth authorization requests (jacquard-oauth AuthRequestData)
CREATE TABLE IF NOT EXISTS oatproxy_auth_requests (
    state TEXT PRIMARY KEY,
    auth_req_data TEXT NOT NULL
);

-- Indexes for common lookups
CREATE INDEX IF NOT EXISTS idx_oatproxy_pending_auths_expires
    ON oatproxy_pending_auths(expires_at);

CREATE INDEX IF NOT EXISTS idx_oatproxy_downstream_clients_expires
    ON oatproxy_downstream_clients(expires_at);

CREATE INDEX IF NOT EXISTS idx_oatproxy_par_data_expires
    ON oatproxy_par_data(expires_at);

CREATE INDEX IF NOT EXISTS idx_oatproxy_refresh_tokens_did
    ON oatproxy_refresh_tokens(account_did);

CREATE INDEX IF NOT EXISTS idx_oatproxy_used_nonces_created
    ON oatproxy_used_nonces(created_at);

-- Persisting the proxy signing key and HMAC secret
CREATE TABLE IF NOT EXISTS oatproxy_signing_key (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    private_key BLOB NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS oatproxy_dpop_hmac_secret (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    hmac_secret BLOB NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Profiles (from app.bsky.actor.profile)
CREATE TABLE IF NOT EXISTS profiles (
    did TEXT PRIMARY KEY,
    handle TEXT,
    display_name TEXT,
    description TEXT,
    avatar_cid TEXT,
    banner_cid TEXT,
    pronouns TEXT,
    website TEXT,
    created_at TEXT,
    updated_at TEXT NOT NULL,
    account_status TEXT NOT NULL DEFAULT 'active',
    account_status_updated_at TEXT,
    last_seen_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_profiles_handle ON profiles(handle);
CREATE INDEX IF NOT EXISTS idx_profiles_created_at ON profiles(created_at);
CREATE INDEX IF NOT EXISTS idx_profiles_account_status ON profiles(account_status);
