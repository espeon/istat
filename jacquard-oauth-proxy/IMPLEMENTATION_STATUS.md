# Implementation Status

## Completed Features ✅

### Core OAuth Flow
The complete OAuth 2.1 authorization code flow with PAR and refresh tokens:

1. **PAR Endpoint** (`/oauth/par`) ✅
   - Accepts pushed authorization requests
   - Stores PAR data with 90 second expiry (RFC 9126)
   - Returns request_uri for use in authorization
   - Supports PKCE (code_challenge, code_challenge_method)

2. **Authorization Endpoint** (`/oauth/authorize`) ✅
   - Supports both PAR (via request_uri) and standard parameters
   - Retrieves and validates PAR data if request_uri provided
   - Stores downstream client info (redirect_uri, state, scope)
   - Uses jacquard-oauth to initiate upstream PDS authentication
   - Resolves user DID to PDS, creates PAR, returns authorization URL
   - Redirects user to their PDS for authentication

3. **Callback Endpoint** (`/oauth/return`) ✅
   - Receives authorization code from upstream PDS
   - Uses jacquard-oauth to exchange code for upstream tokens
   - Retrieves stored downstream client info
   - Generates downstream authorization code
   - Stores pending auth mapping (code → DID + session_id + client info)
   - Redirects back to client with downstream code

4. **Token Endpoint** (`/oauth/token`) ⚠️ **Partially Complete**
   - **Authorization Code Grant**: Currently passes through upstream tokens
     - ✅ Consumes pending authorization
     - ✅ Retrieves upstream session from jacquard-oauth
     - ✅ Stores refresh token mapping
     - ⚠️ Returns upstream tokens (should issue downstream JWTs)
   - **Refresh Token Grant**: Currently passes through upstream tokens
     - ✅ Looks up session by refresh token
     - ✅ Gets current tokens from jacquard-oauth (auto-refreshes if needed)
     - ✅ Updates refresh token mapping if rotated
     - ⚠️ Returns upstream tokens (should issue downstream JWTs)

5. **Revocation Endpoint** (`/oauth/revoke`) ✅
   - Handles token revocation requests
   - Cleans up session data

### Security Features

1. **DPoP Support** ✅
   - Parses DPoP proof JWTs from Authorization header
   - Extracts JWK from proof header
   - Computes RFC 7638 JWK thumbprint (JKT)
   - Supports EC, RSA, and OKP key types
   - Base64url-encoded SHA-256 hash

2. **JWT Infrastructure** ✅
   - `TokenManager::issue_downstream_jwt()` implemented
   - Creates properly signed JWTs with:
     - Standard claims (iss, sub, aud, exp, iat)
     - Scope claim
     - DPoP binding via cnf.jkt claim (RFC 9449)

## Architecture

The proxy implements a dual-layer OAuth architecture:

- **Upstream Layer**: jacquard-oauth manages sessions with the user's PDS
  - Handles DID resolution, PAR creation, token exchange
  - Manages DPoP proofs, token refresh, session storage
  
- **Downstream Layer**: Proxy manages authorization flow with clients
  - Stores client redirect info indexed by DID
  - Generates authorization codes for clients
  - Maps codes to upstream sessions
  - **Current**: Passes through upstream tokens
  - **Target**: Issues separate downstream JWTs (see REMAINING_WORK.md)

## Token Flow (Current Implementation)

**Current behavior**: Pass-through architecture
1. Client authenticates through proxy
2. Proxy exchanges code with PDS, gets upstream tokens
3. Proxy returns upstream tokens directly to client
4. Client uses upstream tokens with PDS directly

**Target behavior**: OATProxy-style (see REMAINING_WORK.md)
1. Client authenticates through proxy
2. Proxy exchanges code with PDS, gets upstream tokens
3. Proxy issues downstream JWT bound to client's DPoP key
4. Client uses downstream JWT with proxy
5. Proxy validates JWT and forwards requests with upstream tokens

## What's Missing for Full OATProxy Implementation

See `REMAINING_WORK.md` for detailed implementation guide. Summary:

1. **Token Endpoint**: Issue downstream JWTs instead of passing through upstream tokens
2. **JWT Validation**: Parse and validate downstream JWTs in API requests
3. **DPoP Proof Creation**: Create DPoP proofs for upstream PDS requests
4. **XRPC Proxy**: Validate downstream tokens, forward requests with upstream auth
5. **Session Storage**: Track active sessions and DPoP keys

Estimated effort: 10-15 hours for complete implementation.

## Remaining TODOs (Optional)

### Optional Features
These features are not required for a functional OAuth proxy since we pass through upstream tokens:

- **XRPC Proxy DPoP** - Create proper DPoP proofs for upstream XRPC requests
  - Only needed if proxy intercepts ongoing API requests
  - Clients can make requests directly to PDS with upstream tokens
  
- **DPoP JKT Extraction** - Parse DPoP header and extract JKT from JWK
  - Would be needed for downstream token binding to client keys
  - Not required when passing through upstream tokens
  
- **JWT Signing** - Implement JWT signing for downstream tokens
  - Would be needed if issuing separate downstream tokens
  - Not required when passing through upstream tokens

### Storage Implementation Required

To use this library, you must implement the `OAuthSessionStore` trait with these methods:

**Session Management:**
- `create_session`, `get_session`, `update_session`, `delete_session`
- `get_by_request_uri`, `get_by_state`, `get_by_dpop_jkt`

**Authorization Flow:**
- `store_pending_auth`, `consume_pending_auth` - Map authorization codes to sessions
- `store_downstream_client_info`, `consume_downstream_client_info` - Store client redirect info
- `store_par_data`, `consume_par_data` - Store PAR requests with 90s expiry

**Refresh Tokens:**
- `store_refresh_token_mapping`, `get_refresh_token_mapping` - Map refresh tokens to sessions

**jacquard-oauth Integration:**
Your store must also implement `jacquard_oauth::authstore::ClientAuthStore`:
- `get_session`, `save_session`, `delete_session` - Upstream session storage
- `get_auth_req_info`, `save_auth_req_info`, `delete_auth_req_info` - Auth request state
1. Implement refresh token flow in the proxy
2. Issue separate downstream tokens instead of passing through upstream tokens
3. Add token revocation synchronization
4. Implement rate limiting and abuse prevention
5. Add comprehensive error handling and logging
6. Implement session cleanup and expiry
7. Add metrics and monitoring

## Testing

To test the implementation, you'll need:
1. Storage implementation (`OAuthSessionStore` trait)
   - Methods: store/consume pending auth
   - Methods: store/consume downstream client info
   - Must also implement jacquard-oauth's `ClientAuthStore`
2. KeyStore implementation (can be placeholder for testing)
3. NonceStore implementation (can be placeholder for testing)

Example flow:
```
GET /oauth/authorize?client_id=did:plc:user123&redirect_uri=http://localhost:3000/callback&state=abc&response_type=code&scope=atproto

→ Stores client info
→ Redirects to: https://pds.example.com/oauth/authorize?...

(User authenticates at PDS)

GET /oauth/return?code=upstream_code&state=upstream_state

→ Exchanges upstream code for tokens
→ Generates downstream code
→ Redirects to: http://localhost:3000/callback?code=downstream_code&state=abc

POST /oauth/token
  grant_type=authorization_code&code=downstream_code

→ Returns: {"access_token": "...", "refresh_token": "...", "token_type": "DPoP", "expires_in": 3600, "scope": "atproto"}
```

## Dependencies

- `jacquard-oauth` (0.9) - Handles upstream PDS OAuth flow
- `jacquard-identity` (0.9) - DID resolution
- `jacquard-common` (0.9) - Common types
- `axum` - HTTP server framework
- `chrono` - Datetime handling
- `serde` - Serialization

## Files Modified

- `src/server.rs` - Main OAuth handlers and routing
- `src/store.rs` - Storage traits and types (added PendingAuth, DownstreamClientInfo)
- `src/error.rs` - Error types
- `src/config.rs` - Configuration
- `Cargo.toml` - Dependencies

## Next Steps

If you want to complete the implementation:

1. **Implement storage layer** - Create a concrete implementation of `OAuthSessionStore` using your preferred storage (Redis, PostgreSQL, etc.)

2. **Add refresh token support** - Implement the refresh_token grant type in the token handler

3. **Consider token strategy** - Decide if you want to pass through tokens or issue separate downstream tokens

4. **Add tests** - Write integration tests for the full OAuth flow

5. **Deploy and test** - Test with real Bluesky/ATProto clients
