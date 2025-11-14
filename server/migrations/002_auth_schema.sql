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
