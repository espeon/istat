# Moderation System Upgrade Guide

## Overview

This upgrade adds a comprehensive moderation system with inline controls, admin panel, and audit logging. **All operations are local to your SQLite database** - no ATProto records are created or modified on PDSes.

## What's New

1. **Admin System**: Environment-based admin bootstrapping
2. **Content Blacklisting**: Block CIDs (emoji blobs, avatars, banners) with reasons
3. **Soft Deletion**: Users and admins can delete content (marked deleted, not removed)
4. **Inline Moderation**: Dropdown controls on status cards
5. **Admin Panel**: Dashboard at `/admin` with stats, search, and filters
6. **Audit Log**: Complete history of all moderation actions

## Database Migrations

Two new migrations will run automatically on server startup:

### Migration 1: `20251122000000_add_moderation.sql`
- Creates `admins` table
- Creates `blacklisted_cids` table with CHECK constraints
- Adds `deleted_at` and `deleted_by` columns to `emojis` and `statuses`
- Creates indexes for performance

### Migration 2: `20251122000001_add_audit_log.sql`
- Creates `moderation_audit_log` table
- Creates indexes for efficient queries
- Logs all moderation actions (blacklist, unblacklist, delete)

**No data loss**: Existing data is preserved, only new columns/tables added.

## Environment Variables

### Required for Admin Access

```bash
# Set the DID of the initial admin user
ADMIN_DID=did:plc:youradmindidhere
```

**How to get your DID:**
1. Log into the application with your account
2. Check the browser console or network requests for your DID
3. Or query: `SELECT did FROM profiles WHERE handle = 'yourhandle.bsky.social'`

### Already Required (reminder)

```bash
# OAuth issuer for JWT validation (should already be set)
OAUTH_ISSUER=http://localhost:3001  # or your public URL
```

## Setup Instructions

### 1. Pull and Build Backend

```bash
cd /path/to/istat
git pull origin your-branch-name
cargo build --release --bin server
```

### 2. Set Environment Variable

```bash
# Add to your .env file or export before running:
export ADMIN_DID=did:plc:your_admin_did_here
export OAUTH_ISSUER=your_issuer_url
```

### 3. Start Server

The migrations run automatically on startup:

```bash
cargo run --release --bin server
# Or if you have a systemd service, restart it
```

You'll see migration logs:
```
Running migration: 20251122000000_add_moderation.sql
Running migration: 20251122000001_add_audit_log.sql
```

### 4. Verify Admin Access

1. Log into the application with your admin account
2. You should see an "admin" badge in the header
3. Click it to access the admin panel at `/admin`

## Frontend Changes

The frontend is automatically rebuilt. Changes include:

- Toast notification system (replaces browser alerts)
- Inline moderation dropdowns on status cards (only visible to admins/owners)
- Admin panel UI at `/admin`
- Audit log viewer

## New API Endpoints

All admin-only except `isAdmin` which any authenticated user can call:

```
POST   /xrpc/vg.nat.istat.moderation.blacklistCid
POST   /xrpc/vg.nat.istat.moderation.removeBlacklist
GET    /xrpc/vg.nat.istat.moderation.listBlacklisted
GET    /xrpc/vg.nat.istat.moderation.listAuditLog
GET    /xrpc/vg.nat.istat.moderation.isAdmin
POST   /xrpc/vg.nat.istat.moji.deleteEmoji
POST   /xrpc/vg.nat.istat.status.deleteStatus
```

## Using the Moderation System

### As an Admin

**Admin Panel (`/admin`):**
1. Navigate to `/admin` (link appears in header when logged in as admin)
2. View stats: total blacklisted items, items added today, top reason
3. Search blacklisted content by CID or details
4. Filter by reason (nudity, gore, harassment, spam, copyright, other)
5. Filter by content type (emoji_blob, avatar, banner)
6. Blacklist new content by entering CID and selecting reason
7. View audit log of all moderation actions

**Inline Moderation:**
1. Find a status in the feed
2. Click the three-dot menu (⋮) on the status card
3. Options:
   - **Blacklist emoji** (admin only): Block the emoji CID with a reason
   - **Delete status**: Soft-delete the status

### As a Content Owner

**Delete Your Own Content:**
1. Find your own status in the feed
2. Click the three-dot menu (⋮)
3. Click "Delete status"
4. Confirm deletion

The status is soft-deleted (marked `deleted_at`, not removed from database).

## How It Works

### Content Filtering

All queries automatically filter out:
1. Content with `deleted_at IS NOT NULL` (soft-deleted)
2. Content whose `blob_cid` is in the `blacklisted_cids` table

Example query pattern:
```sql
WHERE deleted_at IS NULL
  AND blob_cid NOT IN (
    SELECT cid FROM blacklisted_cids
    WHERE content_type = 'emoji_blob'
  )
```

### Blacklisting

When you blacklist a CID:
1. Row inserted into `blacklisted_cids` with reason
2. All future queries exclude content using that CID
3. Statuses using blacklisted emojis are hidden
4. Action logged to audit log

### Soft Deletion

When content is deleted:
1. `deleted_at` set to current timestamp
2. `deleted_by` set to deleter's DID
3. Content filtered from all queries
4. Action logged to audit log
5. **Original record remains in database** (can be undeleted if needed)

### Audit Log

Every moderation action is logged with:
- Moderator DID and handle
- Action type (blacklist_cid, remove_blacklist, delete_emoji, delete_status)
- Target type and ID
- Reason (for blacklists)
- Timestamp

View the log in the admin panel to track all moderation activity.

## Security Notes

### Admin Authorization

- Initial admin bootstrapped via `ADMIN_DID` environment variable
- Additional admins can be added to the `admins` table manually:
  ```sql
  INSERT INTO admins (did, granted_by, notes)
  VALUES ('did:plc:newadmin', 'did:plc:yourAdminDid', 'Added via SQL');
  ```
- All admin endpoints verify JWT and check `admins` table

### Content Owner Authorization

- Delete endpoints check: `user_did == content_did OR is_admin`
- Users can only delete their own content (unless admin)

### No ATProto Record Creation

**Important**: This moderation system only modifies your local SQLite database. It does NOT:
- Create or delete ATProto records on PDSes
- Make any network calls to Bluesky infrastructure
- Modify users' actual ATProto repositories

Content is simply hidden from your local index.

## Troubleshooting

### "Not authorized" when accessing admin panel

**Check:**
1. Is `ADMIN_DID` set correctly in environment?
2. Restart server after setting environment variable
3. Log out and log back in
4. Check server logs for admin table insert

**Verify manually:**
```sql
SELECT * FROM admins;
```

Should show your DID.

### Migrations not running

**Check:**
1. Migration files exist in `server/migrations/`
2. Server has write permissions to database file
3. Check server startup logs for migration errors

**Verify manually:**
```sql
-- Check if tables exist
SELECT name FROM sqlite_master WHERE type='table';
```

Should include: `admins`, `blacklisted_cids`, `moderation_audit_log`

### Content still appearing after blacklist

**Check:**
1. Verify CID is in blacklist table:
   ```sql
   SELECT * FROM blacklisted_cids WHERE cid = 'your_cid_here';
   ```
2. Check content_type matches (emoji_blob, avatar, or banner)
3. Refresh the page (frontend caches data)

### Three-dot menu not appearing

**Check:**
1. You are logged in
2. You are either the content owner OR an admin
3. Status is not already expired
4. Browser console for JavaScript errors

## Rollback (if needed)

If you need to rollback:

```sql
-- Remove moderation tables (LOSES ALL MODERATION DATA)
DROP TABLE IF EXISTS moderation_audit_log;
DROP TABLE IF EXISTS blacklisted_cids;
DROP TABLE IF EXISTS admins;

-- Remove soft delete columns from emojis
ALTER TABLE emojis DROP COLUMN deleted_at;
ALTER TABLE emojis DROP COLUMN deleted_by;

-- Remove soft delete columns from statuses
ALTER TABLE statuses DROP COLUMN deleted_at;
ALTER TABLE statuses DROP COLUMN deleted_by;
```

Then revert to the previous git commit and rebuild.

## Questions?

Check:
1. Server logs for errors
2. Browser console for frontend errors
3. SQLite database directly with `sqlite3 istat.db`
4. Audit log in admin panel for moderation history

The system is designed to be fail-safe: if authorization fails, it denies access rather than allowing unauthorized actions.
