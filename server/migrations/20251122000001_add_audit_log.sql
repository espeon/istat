-- Create audit log table for tracking moderation actions
CREATE TABLE moderation_audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    moderator_did TEXT NOT NULL,
    action TEXT NOT NULL CHECK(action IN ('blacklist_cid', 'remove_blacklist', 'delete_emoji', 'delete_status')),
    target_type TEXT NOT NULL CHECK(target_type IN ('emoji_blob', 'avatar', 'banner', 'status', 'emoji')),
    target_id TEXT NOT NULL,  -- CID for blacklist actions, AT-URI or rkey for deletions
    reason TEXT,  -- For blacklist actions
    reason_details TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,

    -- Indexes for efficient queries
    FOREIGN KEY (moderator_did) REFERENCES admins(did)
);

CREATE INDEX idx_audit_log_moderator ON moderation_audit_log(moderator_did);
CREATE INDEX idx_audit_log_created_at ON moderation_audit_log(created_at DESC);
CREATE INDEX idx_audit_log_action ON moderation_audit_log(action);
