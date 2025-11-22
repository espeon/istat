# Moderation System Implementation

This document describes the moderation system that has been implemented for nyt (formerly istat).

## Overview

The moderation system allows administrators to:
- **Blacklist CIDs**: Block emoji blobs, avatars, or banners from being displayed
- **Soft Delete**: Users can delete their own emoji and statuses
- **Admin Management**: Environment variable-based admin bootstrapping with database-backed permissions

## Database Schema

### New Tables

**`admins`** - Stores admin user DIDs
```sql
CREATE TABLE admins (
    did TEXT PRIMARY KEY,
    granted_by TEXT,  -- NULL for initial admin from env var
    granted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    notes TEXT
);
```

**`blacklisted_cids`** - Tracks blacklisted content
```sql
CREATE TABLE blacklisted_cids (
    cid TEXT PRIMARY KEY,
    reason TEXT NOT NULL CHECK(reason IN ('nudity', 'gore', 'harassment', 'spam', 'copyright', 'other')),
    reason_details TEXT,  -- Additional explanation beyond predefined reason
    content_type TEXT NOT NULL CHECK(content_type IN ('emoji_blob', 'avatar', 'banner')),
    moderator_did TEXT NOT NULL REFERENCES admins(did),
    blacklisted_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### Schema Modifications

**`emojis`** table - Added soft delete columns:
```sql
ALTER TABLE emojis ADD COLUMN deleted_at DATETIME;
ALTER TABLE emojis ADD COLUMN deleted_by TEXT;
```

**`statuses`** table - Added soft delete columns:
```sql
ALTER TABLE statuses ADD COLUMN deleted_at DATETIME;
ALTER TABLE statuses ADD COLUMN deleted_by TEXT;
```

## Backend Endpoints

All endpoints are located in `server/src/xrpc/moderation.rs`.

### Admin-Only Endpoints

**`vg.nat.istat.moderation.blacklistCid`** (POST)
- Blacklist a CID from being displayed
- Request body:
  ```json
  {
    "cid": "bafyrei...",
    "reason": "nudity",  // enum: nudity|gore|harassment|spam|copyright|other
    "reasonDetails": "Optional additional details",
    "contentType": "emoji_blob"  // enum: emoji_blob|avatar|banner
  }
  ```
- Response: `{ "success": true }`
- Errors: `401 Unauthorized`, `403 Forbidden`, `409 Conflict` (already blacklisted)

**`vg.nat.istat.moderation.removeBlacklist`** (POST)
- Remove a CID from the blacklist
- Request body: `{ "cid": "bafyrei..." }`
- Response: `{ "success": true }`
- Errors: `401 Unauthorized`, `403 Forbidden`, `404 Not Found`

**`vg.nat.istat.moderation.listBlacklisted`** (GET)
- List all blacklisted CIDs
- Response:
  ```json
  {
    "blacklisted": [
      {
        "cid": "bafyrei...",
        "reason": "nudity",
        "reasonDetails": "...",
        "contentType": "emoji_blob",
        "moderatorDid": "did:plc:...",
        "blacklistedAt": "2025-11-22T..."
      }
    ]
  }
  ```

### User Endpoints

**`vg.nat.istat.moderation.isAdmin`** (GET)
- Check if the current user is an admin
- Response: `{ "isAdmin": true }`

**`vg.nat.istat.moji.deleteEmoji`** (POST)
- Delete an emoji (owner or admin only)
- Request body: `{ "uri": "at://did:plc:xyz/vg.nat.istat.moji.emoji/rkey" }`
- Response: `{ "success": true }`
- Errors: `401 Unauthorized`, `403 Forbidden` (not owner), `404 Not Found`

**`vg.nat.istat.status.deleteStatus`** (POST)
- Delete a status (owner or admin only)
- Request body: `{ "uri": "at://did:plc:xyz/vg.nat.istat.status.record/rkey" }`
- Response: `{ "success": true }`
- Errors: `401 Unauthorized`, `403 Forbidden` (not owner), `404 Not Found`

## Query Filtering

All existing XRPC endpoints have been updated to filter out blacklisted and deleted content:

- `vg.nat.istat.moji.searchEmoji` - Excludes deleted emoji and emoji with blacklisted blob CIDs
- `vg.nat.istat.status.getStatus` - Excludes deleted statuses and statuses using blacklisted emoji
- `vg.nat.istat.status.listUserStatuses` - Same filtering as above
- `vg.nat.istat.status.listStatuses` - Same filtering as above

## Environment Variables

**Required for Admin Functionality:**
- `ADMIN_DID` - The DID of the initial admin user (e.g., `did:plc:yourdidhere`)
- `OAUTH_ISSUER` - The OAuth issuer URL for JWT validation (defaults to `http://localhost:3001`)

## Setup Instructions

### 1. Set Admin DID

Add your DID to the environment variables:
```bash
export ADMIN_DID="did:plc:yourdidhere"
```

Or add to your `.env` file if using a `.env` loader.

### 2. Run Database Migrations

The migrations will run automatically when you start the server:
```bash
cd server
cargo run --bin server
```

The migration file is located at: `server/migrations/20251122000000_add_moderation.sql`

### 3. Verify Admin Status

Once logged in through the OAuth flow, you can check your admin status by calling:
```bash
curl -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  http://localhost:3000/xrpc/vg.nat.istat.moderation.isAdmin
```

Should return: `{"isAdmin":true}`

### 4. Test Blacklisting

Try blacklisting a CID:
```bash
curl -X POST http://localhost:3000/xrpc/vg.nat.istat.moderation.blacklistCid \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "cid": "bafyreiexample",
    "reason": "other",
    "reasonDetails": "Test blacklist",
    "contentType": "emoji_blob"
  }'
```

## Frontend Integration (TODO)

The following frontend components still need to be implemented:

### 1. Admin Moderation Panel

Create `/admin` route with:
- List of all blacklisted CIDs
- Form to blacklist new CIDs with predefined reason dropdown
- Ability to remove blacklists
- Search by CID, reason, or moderator

### 2. Inline Moderation Controls

Add to `StatusCard.tsx` and emoji views:
- "Blacklist" button (only visible to admins)
- Quick action menu with predefined reasons
- Toast confirmation after action

### 3. Delete Buttons for Own Content

Add to user's own statuses/emoji:
- "Delete" button on hover
- Confirmation dialog
- Refresh feed after deletion

### 4. Moderated Content Placeholders

When content is blacklisted or deleted:
- Show placeholder: "This emoji was removed by moderators"
- Don't show the actual content
- Optional: Show reason if blacklisted (for transparency)

### 5. Admin Indicator

- Badge on admin profiles
- Different UI state when logged in as admin (e.g., header shows "Admin" badge)

## TypeScript Types

TypeScript types have been generated in `frontend/src/lexicons/`. Use them like this:

```typescript
import { moderation } from './lexicons';

// Check if user is admin
const { isAdmin } = await agent.api.vg.nat.istat.moderation.isAdmin();

// Blacklist a CID
await agent.api.vg.nat.istat.moderation.blacklistCid({
  cid: 'bafyreiexample',
  reason: 'nudity',
  reasonDetails: 'Inappropriate content',
  contentType: 'emoji_blob'
});

// Delete your own status
await agent.api.vg.nat.istat.status.deleteStatus({
  uri: 'at://did:plc:xyz/vg.nat.istat.status.record/abc123'
});
```

## Security Considerations

1. **JWT Validation**: All moderation endpoints validate JWTs using the proxy's signing key
2. **Admin Check**: Admin status is verified on every admin-only endpoint call
3. **Ownership Check**: Delete endpoints verify the user owns the content (or is an admin)
4. **Input Validation**: Reason and contentType enums are validated server-side
5. **SQL Injection**: All queries use parameterized statements via sqlx

## Known Limitations

1. **Network Issue**: The server currently cannot compile due to network access issues with the jacquard git repository. This will be resolved when you have proper network access.

2. **No Cascade Delete**: Blacklisting an emoji's blob CID hides the emoji and all statuses using it, but statuses must be explicitly deleted separately if desired.

3. **No Public Moderation Log**: Non-admins cannot see moderation history or reasons.

4. **No Undo for Soft Deletes**: Once deleted, emoji/statuses cannot be undeleted (would need a separate restore endpoint).

## Testing Plan

### Manual Testing Steps

1. **Setup**:
   - Set `ADMIN_DID` environment variable to your DID
   - Start the server
   - Log in via OAuth

2. **Admin Verification**:
   - Call `isAdmin` endpoint - should return `true`
   - Log in with a different non-admin account - should return `false`

3. **Blacklist CID**:
   - Create an emoji
   - Note its `blob_cid`
   - Blacklist the CID using the moderation endpoint
   - Verify the emoji no longer appears in search results
   - Verify statuses using that emoji no longer appear

4. **Delete Own Content**:
   - Create a status
   - Call `deleteStatus` with the status URI
   - Verify the status no longer appears in your feed
   - Verify you get `403 Forbidden` when trying to delete someone else's status

5. **Admin Delete**:
   - Log in as admin
   - Delete another user's emoji/status
   - Should succeed

6. **List Blacklisted**:
   - Call `listBlacklisted` endpoint
   - Verify blacklisted CIDs appear with correct metadata

## Future Enhancements

- [ ] Moderation logs (audit trail of all moderation actions)
- [ ] Bulk blacklist operations
- [ ] User reports system (allow users to report content for moderation)
- [ ] Auto-moderation using content hashing or AI
- [ ] Temporary blacklists (with expiration)
- [ ] Restore/undelete functionality
- [ ] Public moderation transparency (show that content was moderated without details)
- [ ] Grant admin privileges to other users via the API
