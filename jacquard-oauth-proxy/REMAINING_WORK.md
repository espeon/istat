# Remaining Work: Full OATProxy-Style Implementation

## Current State

We have implemented:
- ✅ Complete OAuth 2.1 flow (PAR, authorize, callback, token, revoke)
- ✅ Refresh token support with token rotation
- ✅ DPoP JKT extraction from client proofs (RFC 7638)
- ✅ JWT signing infrastructure (`TokenManager::issue_downstream_jwt()`)
- ✅ Storage traits for all required data

## Architecture Gap

**Current**: We pass upstream PDS tokens directly to clients
**Target**: Issue separate downstream JWTs bound to client DPoP keys

## What Needs Implementation

### 1. Update Token Endpoint (`src/server.rs:handle_token`)

**Current behavior**: Returns upstream tokens from PDS
**Target behavior**: Issue downstream JWTs bound to client's DPoP key

#### Changes needed:

```rust
// In authorization_code grant handler:

// Extract client's DPoP JKT
let dpop_jkt = extract_dpop_jkt(&headers)?;

// Issue downstream JWT (instead of returning upstream token)
let access_token = server
    .token_manager
    .issue_downstream_jwt(
        &pending_auth.account_did,  // sub claim
        &dpop_jkt,                   // bind to client key
        &scope_str,                  // scope from upstream
        3600,                        // 1 hour expiry
        &*server.key_store,
    )
    .await?;

// Generate downstream refresh token (separate from upstream)
let downstream_refresh_token = generate_random_string(64);

// Store mapping: downstream_refresh_token → (account_did, upstream_session_id)
server.session_store
    .store_refresh_token_mapping(
        &downstream_refresh_token,
        pending_auth.account_did.clone(),
        pending_auth.upstream_session_id.clone(),
    )
    .await?;

// Return downstream tokens
TokenResponse {
    access_token,  // downstream JWT
    token_type: "DPoP".to_string(),
    expires_in: 3600,
    refresh_token: Some(downstream_refresh_token),
    scope: scope_str,
}
```

#### In refresh_token grant handler:

```rust
// Extract client's DPoP JKT (may have changed)
let dpop_jkt = extract_dpop_jkt(&headers)?;

// Look up session using downstream refresh token
let (account_did, session_id) = server.session_store
    .get_refresh_token_mapping(&refresh_token)
    .await?
    .ok_or_else(|| Error::InvalidGrant)?;

// Get upstream session (jacquard-oauth auto-refreshes if needed)
let did = Did::new_owned(&account_did)?;
let upstream_session_data = ClientAuthStore::get_session(
    &*server.session_store,
    &did,
    &session_id,
).await?;

// Issue new downstream JWT
let access_token = server.token_manager
    .issue_downstream_jwt(
        &account_did,
        &dpop_jkt,
        &scope_str,
        3600,
        &*server.key_store,
    )
    .await?;

// Generate new downstream refresh token (token rotation)
let new_downstream_refresh = generate_random_string(64);

// Update mapping
server.session_store
    .store_refresh_token_mapping(
        &new_downstream_refresh,
        account_did,
        session_id,
    )
    .await?;

// Return new tokens
```

### 2. Implement JWT Validation (`src/token.rs`)

Add a method to validate incoming downstream JWTs:

```rust
impl TokenManager {
    /// Validate a downstream JWT and extract claims
    pub async fn validate_downstream_jwt(
        &self,
        jwt: &str,
        key_store: &impl KeyStore,
    ) -> Result<DownstreamTokenClaims> {
        use jose_jws::Jws;
        
        // Get signing key for validation
        let signing_key = key_store.get_signing_key().await?;
        
        // Parse and verify JWT
        let jws = Jws::decode_b64(jwt, &signing_key)
            .map_err(|e| Error::InvalidRequest(format!("invalid JWT: {}", e)))?;
        
        // Parse claims
        let claims: DownstreamTokenClaims = serde_json::from_slice(jws.payload())
            .map_err(|e| Error::InvalidRequest(format!("invalid claims: {}", e)))?;
        
        // Verify issuer
        if claims.iss != self.issuer {
            return Err(Error::InvalidRequest("wrong issuer".to_string()));
        }
        
        // Check expiry
        let now = Utc::now().timestamp();
        if claims.exp < now {
            return Err(Error::InvalidRequest("token expired".to_string()));
        }
        
        Ok(claims)
    }
}

#[derive(Debug, Deserialize)]
pub struct DownstreamTokenClaims {
    pub iss: String,
    pub sub: String,  // account DID
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub scope: String,
    pub cnf: ConfirmationClaim,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmationClaim {
    pub jkt: String,  // DPoP JKT
}
```

### 3. Create DPoP Proofs for Upstream Requests (`src/token.rs`)

```rust
impl TokenManager {
    /// Create a DPoP proof for an upstream PDS request
    pub async fn create_upstream_dpop_proof(
        &self,
        method: &str,
        url: &str,
        access_token: Option<&str>,
        nonce: Option<&str>,
        dpop_key: &jose_jwk::Key,
    ) -> Result<String> {
        use jose_jws::{Jws, Header};
        use sha2::{Digest, Sha256};
        use base64::prelude::*;
        
        let now = Utc::now().timestamp();
        let jti = generate_random_string(32);
        
        // Hash access token if provided (for token-bound requests)
        let ath = if let Some(token) = access_token {
            let mut hasher = Sha256::new();
            hasher.update(token.as_bytes());
            let hash = hasher.finalize();
            Some(BASE64_URL_SAFE_NO_PAD.encode(&hash))
        } else {
            None
        };
        
        let mut claims = json!({
            "jti": jti,
            "htm": method,
            "htu": url,
            "iat": now,
        });
        
        if let Some(ath_value) = ath {
            claims["ath"] = json!(ath_value);
        }
        
        if let Some(nonce_value) = nonce {
            claims["nonce"] = json!(nonce_value);
        }
        
        let claims_json = serde_json::to_string(&claims)?;
        
        // Create header with JWK
        let mut header = Header::new();
        header.set_token_type("dpop+jwt");
        header.set_jwk(dpop_key.clone());
        
        // Sign
        let jws = Jws::new_with_b64_payload(
            header,
            claims_json.as_bytes(),
            dpop_key,
        );
        
        Ok(jws.encode_b64()?)
    }
}
```

### 4. Update XRPC Proxy Handler (`src/server.rs:handle_xrpc_proxy`)

**Current**: Tries to use our OAuthSession (doesn't work)
**Target**: Validate downstream JWT, forward with upstream DPoP

```rust
async fn handle_xrpc_proxy<S, K, N>(
    State(server): State<OAuthProxyServer<S, K, N>>,
    method: Method,
    uri: http::Uri,
    headers: HeaderMap,
    body: String,
) -> Result<Response>
where
    S: OAuthSessionStore + ClientAuthStore + Clone + 'static,
    K: KeyStore + Clone + 'static,
    N: NonceStore + Clone + 'static,
{
    // 1. Extract and validate downstream JWT
    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(Error::Unauthorized)?;
    
    let token = auth_header
        .strip_prefix("DPoP ")
        .ok_or(Error::Unauthorized)?;
    
    let claims = server.token_manager
        .validate_downstream_jwt(token, &*server.key_store)
        .await?;
    
    // 2. Verify DPoP binding
    let dpop_jkt = extract_dpop_jkt(&headers)?;
    if dpop_jkt != claims.cnf.jkt {
        return Err(Error::InvalidRequest("DPoP key mismatch".to_string()));
    }
    
    // 3. Look up upstream session
    // NOTE: We need to store a mapping from account_did → active session_id
    // This is a gap in current storage - need to add:
    // - store_active_session(did, session_id)
    // - get_active_session(did) -> session_id
    
    let session_id = server.session_store
        .get_active_session(&claims.sub)
        .await?
        .ok_or(Error::SessionNotFound)?;
    
    let did = Did::new_owned(&claims.sub)?;
    let upstream_session_data = ClientAuthStore::get_session(
        &*server.session_store,
        &did,
        &session_id,
    ).await?
    .ok_or(Error::SessionNotFound)?;
    
    // 4. Build upstream URL
    let upstream_url = format!(
        "{}{}",
        upstream_session_data.host_url,
        uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("")
    );
    
    // 5. Get upstream DPoP key
    // NOTE: Need to store this when creating session
    let upstream_dpop_key = server.key_store
        .get_dpop_key(&upstream_session_data.dpop_jkt)
        .await?
        .ok_or(Error::SessionNotFound)?;
    
    // 6. Create DPoP proof for upstream request
    let dpop_proof = server.token_manager
        .create_upstream_dpop_proof(
            method.as_str(),
            &upstream_url,
            Some(upstream_session_data.token_set.access_token.as_ref()),
            upstream_session_data.dpop_nonce.as_deref(),
            &upstream_dpop_key,
        )
        .await?;
    
    // 7. Forward request to PDS
    let client = reqwest::Client::new();
    let mut request = client
        .request(method, &upstream_url)
        .header("Authorization", format!("DPoP {}", upstream_session_data.token_set.access_token))
        .header("DPoP", dpop_proof);
    
    // Copy relevant headers (skip auth/dpop)
    for (name, value) in headers.iter() {
        if !["host", "authorization", "dpop"].contains(&name.as_str()) {
            request = request.header(name, value);
        }
    }
    
    if !body.is_empty() {
        request = request.body(body);
    }
    
    // Send request
    let response = request.send().await
        .map_err(|e| Error::NetworkError(e.to_string()))?;
    
    // Handle DPoP nonce updates
    if let Some(new_nonce) = response.headers().get("DPoP-Nonce") {
        // TODO: Store new nonce for next request
    }
    
    // Return response
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.text().await
        .map_err(|e| Error::NetworkError(e.to_string()))?;
    
    let mut response_builder = axum::http::Response::builder().status(status);
    for (name, value) in headers.iter() {
        response_builder = response_builder.header(name, value);
    }
    
    Ok(response_builder.body(body.into())?)
}
```

### 5. Storage Additions Needed

Add to `OAuthSessionStore` trait in `src/store.rs`:

```rust
/// Store active session mapping (DID → session_id)
async fn store_active_session(&self, did: &str, session_id: String) -> Result<()>;

/// Get active session for a DID
async fn get_active_session(&self, did: &str) -> Result<Option<String>>;

/// Store DPoP key for a session
async fn store_session_dpop_key(&self, session_id: &str, dpop_jkt: String, key: jose_jwk::Key) -> Result<()>;

/// Get DPoP key for a session
async fn get_session_dpop_key(&self, session_id: &str) -> Result<Option<(String, jose_jwk::Key)>>;

/// Store DPoP nonce for a session
async fn update_session_dpop_nonce(&self, session_id: &str, nonce: String) -> Result<()>;

/// Get DPoP nonce for a session
async fn get_session_dpop_nonce(&self, session_id: &str) -> Result<Option<String>>;
```

### 6. Error Handling

Add to `src/error.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // ... existing variants ...
    
    #[error("unauthorized")]
    Unauthorized,
}
```

## Testing Strategy

Once implemented, test flow:

1. **Token issuance**: Verify downstream JWT has correct claims and DPoP binding
2. **Token validation**: Verify JWT signature, expiry, and DPoP JKT match
3. **XRPC proxy**: Make authenticated requests through proxy to PDS
4. **Refresh flow**: Verify token rotation and session continuity
5. **DPoP proofs**: Verify upstream requests include valid DPoP proofs

## Key Differences from Pass-Through Architecture

| Aspect | Pass-Through (Current) | OATProxy-Style (Target) |
|--------|----------------------|-------------------------|
| Downstream tokens | Upstream PDS tokens | Proxy-issued JWTs |
| Token binding | None | Bound to client DPoP key |
| Token lifetime | PDS-controlled | Proxy-controlled |
| XRPC requests | Direct to PDS | Through proxy |
| Upstream DPoP | N/A | Proxy creates proofs |
| Session storage | Minimal | Full state tracking |

## Implementation Order

1. Start with token endpoint JWT issuance (simplest)
2. Add JWT validation
3. Update storage for session tracking
4. Implement DPoP proof creation
5. Wire up XRPC proxy (most complex)
6. Test end-to-end flow

## Estimated Complexity

- Token endpoint updates: **Easy** (1-2 hours)
- JWT validation: **Easy** (1 hour)
- Storage additions: **Medium** (2-3 hours)
- DPoP proof creation: **Medium** (2-3 hours)
- XRPC proxy: **Hard** (4-6 hours)

**Total**: ~10-15 hours for complete implementation

## Alternative: Hybrid Approach

If full XRPC proxying isn't needed immediately:

1. Implement JWT issuance (tokens are proxy-issued)
2. Skip XRPC proxy (clients use JWTs directly with PDS)
3. Add XRPC proxy later when needed

This gives you proper token architecture without the proxy complexity.
