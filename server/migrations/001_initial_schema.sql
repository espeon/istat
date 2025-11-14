CREATE TABLE IF NOT EXISTS emojis (
    at TEXT PRIMARY KEY NOT NULL,
    did TEXT NOT NULL,
    blob_cid TEXT NOT NULL,
    alt_text TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_emojis_did ON emojis(did);
CREATE INDEX IF NOT EXISTS idx_emojis_created_at ON emojis(created_at);

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
