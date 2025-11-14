# Quick Start: What Works Now

## Current Status

You have a **functional OAuth 2.1 proxy** that implements the complete authorization flow. It's currently using a simpler "pass-through" architecture where clients get upstream tokens directly.

## What You Can Do Right Now

### 1. Run the OAuth Flow

```
Client → /oauth/par (optional)
      → /oauth/authorize?client_id=did:plc:...
      → (redirect to PDS)
      → (user authenticates)
      → /oauth/return (callback from PDS)
      → (redirect back to client with code)
Client → /oauth/token (exchange code for tokens)
      ← receives upstream PDS tokens

Client uses tokens directly with PDS
```

### 2. Supported Features

- ✅ PAR (Pushed Authorization Requests) with 90s expiry
- ✅ Standard OAuth authorize flow
- ✅ DID → PDS resolution via jacquard-oauth
- ✅ Token exchange (authorization_code grant)
- ✅ Token refresh (refresh_token grant with rotation)
- ✅ Token revocation
- ✅ DPoP JKT extraction from client proofs
- ✅ PKCE support

### 3. What You Need to Implement

To use this library, you must provide implementations for:

**`OAuthSessionStore` trait** - Your storage backend (Redis, PostgreSQL, etc.):
```rust
// Basic session CRUD
create_session, get_session, update_session, delete_session
get_by_request_uri, get_by_state, get_by_dpop_jkt

// Authorization flow
store_pending_auth, consume_pending_auth
store_downstream_client_info, consume_downstream_client_info

// PAR support
store_par_data, consume_par_data

// Refresh tokens
store_refresh_token_mapping, get_refresh_token_mapping
```

**jacquard-oauth's `ClientAuthStore` trait** - For upstream sessions:
```rust
get_session, save_session, delete_session
get_auth_req_info, save_auth_req_info, delete_auth_req_info
```

**`KeyStore` trait** - For signing keys:
```rust
get_signing_key()  // For signing downstream JWTs
create_dpop_key(), get_dpop_key()  // For DPoP proofs
```

**`NonceStore` trait** - For DPoP replay protection:
```rust
check_and_consume_nonce, generate_nonce, cleanup_expired
```

### 4. Example Usage

```rust
use jacquard_oauth_proxy::OAuthProxyServer;

// Create your storage implementations
let session_store = Arc::new(MySessionStore::new());
let key_store = Arc::new(MyKeyStore::new());
let nonce_store = Arc::new(MyNonceStore::new());

// Configure the proxy
let config = ProxyConfig {
    host: "https://oauth.example.com".to_string(),
    client_metadata: ClientMetadata {
        client_id: "https://oauth.example.com/client-metadata.json".to_string(),
        // ... other metadata
    },
    scope: vec!["atproto".to_string()],
};

// Build the server
let server = OAuthProxyServer::builder()
    .config(config)
    .session_store(session_store)
    .key_store(key_store)
    .nonce_store(nonce_store)
    .build()?;

// Get the router
let app = server.router();

// Run with axum
axum::Server::bind(&"0.0.0.0:3000".parse()?)
    .serve(app.into_make_service())
    .await?;
```

## Upgrade Path to Full OATProxy

When you're ready to implement the full OATProxy-style architecture:

1. **Read `REMAINING_WORK.md`** - Detailed implementation guide
2. **Update token endpoint** - Issue JWTs instead of pass-through (2-3 hours)
3. **Add JWT validation** - Parse and validate downstream tokens (1 hour)
4. **Implement DPoP creation** - Create proofs for upstream (2-3 hours)
5. **Wire up XRPC proxy** - Forward authenticated requests (4-6 hours)

Total estimated time: 10-15 hours

## Current Limitations

- ❌ Does not issue separate downstream tokens (passes through upstream)
- ❌ Does not proxy XRPC requests (clients talk directly to PDS)
- ❌ Does not create DPoP proofs for upstream requests
- ❌ Does not validate downstream JWTs

These are all documented in `REMAINING_WORK.md` with implementation details.

## Testing Checklist

Before deploying, test:

- [ ] PAR flow with request_uri
- [ ] Standard authorize flow without PAR
- [ ] Token exchange with valid authorization code
- [ ] Token refresh with valid refresh token
- [ ] Token refresh with token rotation
- [ ] Invalid/expired authorization codes rejected
- [ ] Invalid/expired refresh tokens rejected
- [ ] DPoP binding (client must use same key)

## Next Steps

1. Implement storage backends for your infrastructure
2. Deploy and test OAuth flow end-to-end
3. When ready, implement full OATProxy features from `REMAINING_WORK.md`

## Questions?

- **Architecture**: See `IMPLEMENTATION_STATUS.md`
- **Remaining work**: See `REMAINING_WORK.md`
- **Code structure**: See inline documentation in `src/`
