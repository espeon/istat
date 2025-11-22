-- Admins table
CREATE TABLE admins (
    did TEXT PRIMARY KEY,
    granted_by TEXT,  -- NULL for initial admin from env var
    granted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    notes TEXT
);

-- Blacklisted CIDs table
CREATE TABLE blacklisted_cids (
    cid TEXT PRIMARY KEY,
    reason TEXT NOT NULL CHECK(reason IN ('nudity', 'gore', 'harassment', 'spam', 'copyright', 'other')),
    reason_details TEXT,  -- Additional explanation beyond predefined reason
    content_type TEXT NOT NULL CHECK(content_type IN ('emoji_blob', 'avatar', 'banner')),
    moderator_did TEXT NOT NULL REFERENCES admins(did),
    blacklisted_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_blacklisted_cids_moderator ON blacklisted_cids(moderator_did);
CREATE INDEX idx_blacklisted_cids_content_type ON blacklisted_cids(content_type);

-- Add soft delete columns to emojis
ALTER TABLE emojis ADD COLUMN deleted_at DATETIME;
ALTER TABLE emojis ADD COLUMN deleted_by TEXT;  -- DID of who deleted it (user or admin)

-- Add soft delete columns to statuses
ALTER TABLE statuses ADD COLUMN deleted_at DATETIME;
ALTER TABLE statuses ADD COLUMN deleted_by TEXT;

CREATE INDEX idx_emojis_deleted_at ON emojis(deleted_at);
CREATE INDEX idx_statuses_deleted_at ON statuses(deleted_at);
