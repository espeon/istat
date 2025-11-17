# jacquard-oatproxy: deep dive

an oauth 2.1 authorization server that sits between atproto clients and personal data servers (pdses), enabling confidential client mode with extended session lifetimes.

---

## table of contents

1. [the problem](#the-problem)
2. [the solution](#the-solution)
3. [architecture overview](#architecture-overview)
4. [the dual oauth role](#the-dual-oauth-role)
5. [oauth 2.1 flow deep dive](#oauth-21-flow-deep-dive)
6. [dpop: demonstration of proof-of-possession](#dpop-demonstration-of-proof-of-possession)
7. [session management](#session-management)
8. [token lifecycle](#token-lifecycle)
9. [xrpc proxying](#xrpc-proxying)
10. [storage abstraction](#storage-abstraction)
11. [jwt signing and validation](#jwt-signing-and-validation)
12. [jwk thumbprints (jkt)](#jwk-thumbprints-jkt)
13. [security mechanisms](#security-mechanisms)
14. [implementation guide](#implementation-guide)
15. [configuration](#configuration)
16. [metadata endpoints](#metadata-endpoints)
17. [error handling](#error-handling)

---

## the problem

atproto's oauth implementation has a fundamental limitation: public clients (like browser apps or mobile apps that can't securely store credentials) get session tokens that expire after about **1 week**. this is a security feature - if an attacker steals a token from a public client, the damage is limited.

but for legitimate applications, this means users get logged out frequently. confidential clients (servers that can securely store credentials) get much longer sessions - about **1 year** - but most atproto sdks expect to talk directly to a pds, not through a backend proxy.

additionally, atproto uses dpop (demonstration of proof-of-possession) to bind tokens to specific keys, adding another layer of complexity for developers.

## the solution

jacquard-oatproxy is a transparent middleman that:

1. **acts as a confidential oauth client** to the user's pds, getting long-lived sessions
2. **acts as an oauth server** to your application, issuing short-lived jwts
3. **proxies xrpc requests** from your app to the pds, handling all the dpop complexity
4. **manages token refresh** automatically so sessions stay alive for ~1 year

from your app's perspective, the proxy looks like a pds. from the pds's perspective, the proxy is a well-behaved confidential client. you get the best of both worlds: long sessions and simple sdk integration.

## architecture overview

the proxy operates at three layers:

### layer 1: oauth server (downstream)
your application connects to the proxy using standard oauth 2.1 flows. the proxy implements:
- pushed authorization requests (par)
- authorization endpoint
- token endpoint (code exchange + refresh)
- token revocation
- oauth 2.1 metadata endpoints

### layer 2: oauth client (upstream)
the proxy maintains its own oauth sessions with users' pdses using confidential client credentials. it:
- performs par with the upstream pds
- exchanges authorization codes for long-lived tokens
- maintains dpop key pairs for each session
- automatically refreshes tokens before expiry
- creates dpop proofs for every upstream request

### layer 3: xrpc proxy
when your app makes xrpc requests, the proxy:
- validates your jwt and dpop proof
- looks up the upstream session
- creates a new dpop proof for the pds
- forwards the request with the upstream token
- handles dpop nonce requirements
- returns the pds response to your app

this three-layer design means oauth complexity is hidden from both your application code and your users.

## the dual oauth role

understanding the proxy's dual role is key to understanding how it works.

### downstream relationship (app ↔ proxy)

**the proxy is the oauth server**

your application treats the proxy exactly like it would treat a pds. it:
- discovers oauth endpoints via `.well-known/oauth-authorization-server`
- initiates oauth with par
- receives authorization codes
- exchanges codes for jwts
- refreshes tokens using refresh tokens
- makes xrpc requests with dpop proofs

from your app's perspective, the proxy is just another atproto pds with normal oauth.

**token characteristics**
- **type**: jwt (json web token)
- **algorithm**: es256 (p256 ecdsa)
- **lifetime**: 1 hour
- **binding**: dpop (bound to client's key via jkt)
- **claims**: standard oauth + atproto scope + dpop confirmation

### upstream relationship (proxy ↔ pds)

**the proxy is an oauth client**

to the user's actual pds, the proxy appears as a confidential oauth client. it:
- registers as a confidential client (or uses pre-configured credentials)
- uses client credentials authentication
- requests long-lived tokens
- maintains dpop key pairs per session
- handles token refresh automatically
- creates dpop proofs for every request

from the pds's perspective, the proxy is a trusted backend service with proper authentication.

**token characteristics**
- **type**: bearer token with dpop binding
- **lifetime**: ~1 year (confidential client privilege)
- **refresh**: automatic via jacquard-oauth library
- **binding**: dpop (bound to proxy's key per session)
- **storage**: proxy's responsibility (via storage traits)

### why this matters

this dual role is what makes the magic work:

1. **session extension**: your app gets the benefit of 1-year sessions without being a confidential client
2. **sdk compatibility**: your app can use any atproto sdk that expects to talk to a pds
3. **security isolation**: the proxy handles sensitive long-lived credentials; your app only sees short-lived jwts
4. **transparent operation**: users authenticate once via their pds; the proxy never sees passwords

## oauth 2.1 flow deep dive

let's walk through the complete oauth flow with every detail.

### step 1: pushed authorization request (par)

**endpoint**: `POST /oauth/par`

your app initiates oauth by pushing its authorization request parameters to the proxy. this is required by oauth 2.1 (par is mandatory).

**request from app**:
```http
POST /oauth/par HTTP/1.1
Host: proxy.example.com
Content-Type: application/x-www-form-urlencoded
DPoP: eyJ0eXAiOiJkcG9wK2p3dCIsImFsZyI6IkVTMjU2IiwiandrIjp7Imt0eSI6Ik...

client_id=https://proxy.example.com/oauth/downstream/client-metadata.json
&redirect_uri=https://myapp.example.com/callback
&response_type=code
&scope=atproto+transition:generic
&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM
&code_challenge_method=S256
&state=af0ifjsldkj
```

**what the proxy does**:

1. **validates dpop proof** using `dpop_verifier` crate
   - checks jwt signature against embedded public key
   - verifies `htm` matches `POST`
   - verifies `htu` matches the par endpoint url
   - checks `iat` timestamp (max 60 seconds old)
   - validates `jti` hasn't been used (replay protection)
   - optionally validates nonce if one was issued

2. **generates dpop nonce** (if client didn't provide one)
   - uses hmac-based stateless nonce generation
   - includes client key thumbprint, endpoint, and timestamp
   - 5-minute max age
   - no server state needed

3. **computes jkt** (jwk thumbprint) of client's dpop key
   - extracts public key from dpop proof header
   - canonicalizes key parameters (crv, kty, x, y for ec keys)
   - computes sha-256 hash of canonical json
   - base64url-encodes result
   - used later for session lookup

4. **stores par data** with 90-second ttl
   - maps random `request_uri` → par parameters
   - includes client metadata, pkce challenge, redirect uri
   - stored via `OAuthSessionStore::store_par_data`

5. **returns request_uri**:
```json
{
  "request_uri": "urn:ietf:params:oauth:request_uri:6esc_11acc5bdc5",
  "expires_in": 90
}
```

### step 2: authorization request

**endpoint**: `GET /oauth/authorize`

your app sends the user to the authorization endpoint with the `request_uri` from par.

**request from app**:
```http
GET /oauth/authorize?request_uri=urn:ietf:params:oauth:request_uri:6esc_11acc5bdc5 HTTP/1.1
Host: proxy.example.com
```

**what the proxy does**:

1. **consumes par data** using the `request_uri`
   - retrieves stored par parameters
   - validates ttl hasn't expired (90 seconds)
   - par data is single-use (consumed on retrieval)

2. **resolves user's pds**
   - if user provided a did/handle, resolves their pds url
   - otherwise uses default pds from config
   - uses `jacquard-identity` for did/handle resolution

3. **initiates upstream oauth flow**
   - uses `jacquard-oauth` library as oauth client
   - performs par with the user's actual pds
   - includes proxy's confidential client credentials
   - requests `atproto` scope
   - includes dpop support indicators

4. **generates state parameter** for oauth callback
   - maps state → (user did, downstream client info)
   - stored via `OAuthSessionStore::store_pending_auth`

5. **redirects user to pds**:
```http
HTTP/1.1 302 Found
Location: https://user-pds.example.com/oauth/authorize?request_uri=urn:ietf:params:oauth:request_uri:upstream_xyz&state=proxy_generated_state
```

**user authenticates at pds**: the user logs in at their actual pds. the proxy never sees credentials.

### step 3: oauth callback

**endpoint**: `GET /oauth/return`

after the user authenticates, the pds redirects back to the proxy with an authorization code.

**request from pds**:
```http
GET /oauth/return?code=UPSTREAM_AUTH_CODE&state=proxy_generated_state HTTP/1.1
Host: proxy.example.com
```

**what the proxy does**:

1. **validates state parameter**
   - looks up pending auth by state
   - retrieves user did and downstream client info
   - consumes state (single use)

2. **exchanges code for upstream tokens**
   - uses `jacquard-oauth::OAuthClient::finish_authorization`
   - sends authorization code to pds's token endpoint
   - includes dpop proof signed with newly generated key pair
   - receives long-lived access token + refresh token (~1 year)
   - receives dpop confirmation (jkt of the key)

3. **stores upstream session**
   - saves access token, refresh token, expiry
   - saves dpop key pair (private key for future proofs)
   - saves dpop nonce if pds provided one
   - maps (user did, session_id) → upstream session
   - session_id is unique identifier for this oauth session

4. **generates downstream authorization code**
   - creates random code for downstream client
   - maps code → (user did, upstream session_id, pkce verifier)
   - stored via `OAuthSessionStore::store_downstream_client_info`
   - 10-minute expiry

5. **redirects to app with code**:
```http
HTTP/1.1 302 Found
Location: https://myapp.example.com/callback#code=DOWNSTREAM_AUTH_CODE&state=af0ifjsldkj
```

note the use of hash fragment (`#`) - this follows atproto's oauth conventions.

### step 4: token exchange

**endpoint**: `POST /oauth/token`

your app exchanges the authorization code for tokens.

**request from app**:
```http
POST /oauth/token HTTP/1.1
Host: proxy.example.com
Content-Type: application/x-www-form-urlencoded
DPoP: eyJ0eXAiOiJkcG9wK2p3dCIsImFsZyI6IkVTMjU2IiwiandrIjp7Imt0eSI6Ik...

grant_type=authorization_code
&code=DOWNSTREAM_AUTH_CODE
&redirect_uri=https://myapp.example.com/callback
&code_verifier=dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk
&client_id=https://proxy.example.com/oauth/downstream/client-metadata.json
```

**what the proxy does**:

1. **validates authorization code**
   - looks up code in store
   - checks expiry (10 minutes)
   - verifies redirect_uri matches
   - validates pkce: sha256(code_verifier) == code_challenge
   - consumes code (single use)

2. **validates dpop proof**
   - same validation as in par step
   - extracts client's dpop public key
   - computes jkt of client's key

3. **retrieves upstream session**
   - uses (user did, session_id) from consumed code
   - loads upstream access token and refresh token
   - verifies upstream session is still valid

4. **generates downstream jwt** (see [jwt signing](#jwt-signing-and-validation))
   - creates jwt with es256 algorithm
   - includes claims:
     ```json
     {
       "iss": "https://proxy.example.com",
       "sub": "did:plc:user123",
       "aud": "https://proxy.example.com",
       "exp": 1700000000,
       "iat": 1699996400,
       "scope": "atproto transition:generic",
       "cnf": {
         "jkt": "base64url_thumbprint_of_client_dpop_key"
       }
     }
     ```
   - signs with proxy's long-lived p256 private key
   - 1 hour expiry

5. **generates refresh token**
   - creates random 64-byte token
   - maps refresh_token → (user did, session_id)
   - stored via `OAuthSessionStore::store_refresh_token_mapping`
   - same lifetime as upstream token (~1 year)

6. **stores active session**
   - maps user did → session_id
   - allows future lookups by did
   - stored via `OAuthSessionStore::store_active_session`

7. **stores session dpop key**
   - associates client's jkt with session
   - allows session lookup by dpop key
   - stored via `OAuthSessionStore::store_session_dpop_key`

8. **returns tokens to app**:
```json
{
  "access_token": "eyJhbGciOiJFUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "DPoP",
  "expires_in": 3600,
  "refresh_token": "random_base64_string",
  "scope": "atproto transition:generic",
  "sub": "did:plc:user123"
}
```

### step 5: token refresh

**endpoint**: `POST /oauth/token` (with refresh_token grant)

when your app's jwt expires (after 1 hour), it can get a new one using the refresh token.

**request from app**:
```http
POST /oauth/token HTTP/1.1
Host: proxy.example.com
Content-Type: application/x-www-form-urlencoded
DPoP: eyJ0eXAiOiJkcG9wK2p3dCIsImFsZyI6IkVTMjU2IiwiandrIjp7Imt0eSI6Ik...

grant_type=refresh_token
&refresh_token=random_base64_string
&client_id=https://proxy.example.com/oauth/downstream/client-metadata.json
```

**what the proxy does**:

1. **validates refresh token**
   - looks up (user did, session_id) by refresh token
   - verifies token hasn't been revoked

2. **validates dpop proof**
   - may be a new dpop key (client can rotate)
   - computes new jkt if key changed

3. **checks upstream session**
   - retrieves upstream session by (did, session_id)
   - if upstream token is close to expiry, refreshes it automatically
   - `jacquard-oauth` handles upstream refresh transparently
   - new upstream tokens stored back to session store

4. **generates new downstream jwt**
   - creates new jwt with 1 hour expiry
   - uses potentially new client dpop key jkt

5. **rotates refresh token**
   - generates new refresh token
   - maps new token → (did, session_id)
   - deletes old refresh token from store
   - prevents refresh token replay attacks

6. **updates session dpop key**
   - if client rotated dpop keys, updates stored jkt
   - allows session lookup by new key

7. **returns new tokens**:
```json
{
  "access_token": "new_jwt...",
  "token_type": "DPoP",
  "expires_in": 3600,
  "refresh_token": "new_random_token",
  "scope": "atproto transition:generic",
  "sub": "did:plc:user123"
}
```

### step 6: token revocation

**endpoint**: `POST /oauth/revoke`

your app can explicitly end the session.

**request from app**:
```http
POST /oauth/revoke HTTP/1.1
Host: proxy.example.com
Content-Type: application/x-www-form-urlencoded
DPoP: eyJ0eXAiOiJkcG9wK2p3dCIsImFsZyI6IkVTMjU2IiwiandrIjp7Imt0eSI6Ik...

token=random_base64_string
&token_type_hint=refresh_token
&client_id=https://proxy.example.com/oauth/downstream/client-metadata.json
```

**what the proxy does**:

1. **validates dpop proof**
   - ensures request comes from legitimate client

2. **looks up session by dpop jkt**
   - uses jkt from dpop proof to find session
   - this ensures only the actual token holder can revoke

3. **deletes all session data**
   - removes refresh token mapping
   - removes active session mapping
   - removes dpop key mapping
   - optionally revokes upstream session with pds

4. **returns success**:
```http
HTTP/1.1 200 OK
```

note: per oauth spec, revocation endpoint always returns success even if token was already revoked.

## dpop: demonstration of proof-of-possession

dpop is a critical security mechanism that binds tokens to specific cryptographic keys. let's understand how it works in detail.

### what is dpop?

dpop (rfc 9449) prevents token theft by requiring the client to prove possession of a private key with every request. even if an attacker intercepts a token, they can't use it without the corresponding private key.

traditional bearer tokens are like cash - anyone who has one can spend it. dpop tokens are like checks - you need both the check and a signature to cash it.

### dpop in the proxy

the proxy uses dpop in **both directions**:

1. **downstream dpop** (app → proxy): your app proves possession of its key
2. **upstream dpop** (proxy → pds): the proxy proves possession of its key

let's examine both.

### downstream dpop (app → proxy)

**key generation**

your app generates an ephemeral p256 ecdsa key pair at startup or when initiating oauth:

```javascript
// browser example
const keyPair = await crypto.subtle.generateKey(
  {
    name: "ECDSA",
    namedCurve: "P-256",
  },
  true, // extractable
  ["sign", "verify"]
);
```

**dpop proof structure**

for every request to the proxy, your app creates a dpop proof jwt:

```json
// header
{
  "typ": "dpop+jwt",
  "alg": "ES256",
  "jwk": {
    "kty": "EC",
    "crv": "P-256",
    "x": "base64url_encoded_x_coordinate",
    "y": "base64url_encoded_y_coordinate"
  }
}

// payload
{
  "jti": "unique_request_id", // prevents replay
  "htm": "POST", // http method
  "htu": "https://proxy.example.com/oauth/token", // http uri (no query/fragment)
  "iat": 1699996400, // issued at timestamp
  "nonce": "server_provided_nonce" // optional, from DPoP-Nonce header
}
```

the proof is signed with the private key and sent in the `DPoP` header:

```http
DPoP: eyJ0eXAiOiJkcG9wK2p3dCIsImFsZyI6IkVTMjU2IiwiandrIjp7Imt0eSI6IkVDIiwiY3J2IjoiUC0yNTYiLCJ4IjoiLi4uIiwieSI6Ii4uLiJ9fQ.eyJqdGkiOiIuLi4iLCJodG0iOiJQT1NUIiwiaHR1IjoiaHR0cHM6Ly9wcm94eS5leGFtcGxlLmNvbS9vYXV0aC90b2tlbiIsImlhdCI6MTY5OTk5NjQwMH0.signature
```

**dpop validation (proxy side)**

when the proxy receives a request with dpop, it validates:

1. **jwt structure**
   - must be well-formed jwt
   - header must have `typ: "dpop+jwt"`
   - header must have `alg: "ES256"` (or other allowed algorithm)
   - header must contain `jwk` with public key

2. **signature verification**
   - extracts public key from `jwk` header claim
   - verifies jwt signature using that public key
   - ensures the requester controls the private key

3. **temporal validity**
   - checks `iat` timestamp is recent (max 60 seconds old)
   - prevents replay of old proofs

4. **http binding**
   - verifies `htm` matches actual http method
   - verifies `htu` matches actual request uri (excluding query/fragment)
   - prevents proof reuse across endpoints

5. **nonce validation** (if nonce was issued)
   - verifies `nonce` in payload matches expected value
   - nonces are stateless, validated via hmac
   - includes client jkt, endpoint, and timestamp in hmac input
   - max 5-minute nonce age

6. **replay protection**
   - verifies `jti` hasn't been seen before
   - stores used jtis in session store
   - `OAuthSessionStore::check_and_consume_nonce` returns false if replayed

7. **jkt computation**
   - extracts jwk from header
   - computes sha-256 thumbprint (see [jwk thumbprints](#jwk-thumbprints-jkt))
   - uses jkt for session lookup and binding verification

**dpop binding in jwts**

when the proxy issues a jwt to your app, it includes the jkt in the token:

```json
{
  "cnf": {
    "jkt": "base64url_sha256_thumbprint_of_client_key"
  }
}
```

later, when your app makes xrpc requests, the proxy:
1. validates the jwt signature
2. extracts `cnf.jkt` from jwt claims
3. computes jkt from the dpop proof in the request
4. verifies they match

this ensures the jwt can only be used by the client that holds the private key.

### upstream dpop (proxy → pds)

**key generation**

the proxy generates a unique p256 ecdsa key pair for each upstream session:

```rust
use p256::ecdsa::SigningKey;
let signing_key = SigningKey::random(&mut OsRng);
let verifying_key = signing_key.verifying_key();
```

**key storage**

the key pair is stored alongside the upstream session:
- private key: used to sign dpop proofs
- public key: sent to pds in dpop proofs
- jkt: used to verify dpop binding with pds

**dpop proof creation**

for every xrpc request to the pds, the proxy creates a dpop proof:

```json
// header
{
  "typ": "dpop+jwt",
  "alg": "ES256",
  "jwk": {
    "kty": "EC",
    "crv": "P-256",
    "x": "...",
    "y": "..."
  }
}

// payload
{
  "jti": "unique_request_id",
  "htm": "GET",
  "htu": "https://user-pds.example.com/xrpc/app.bsky.feed.getTimeline",
  "iat": 1699996400,
  "ath": "base64url_sha256_of_access_token", // binds proof to token
  "nonce": "pds_provided_nonce" // if pds requires it
}
```

the `ath` claim is critical - it's the sha-256 hash of the access token, base64url-encoded. this binds the dpop proof to the specific access token, preventing proof reuse with other tokens.

**nonce handling**

pdses may require dpop nonces to prevent replay attacks. the flow:

1. **first request**: proxy sends dpop proof without nonce
2. **pds response**: returns 401 or 400 with `DPoP-Nonce` header
3. **retry**: proxy stores nonce and retries with nonce in proof
4. **subsequent requests**: proxy includes stored nonce
5. **nonce refresh**: if nonce expires, repeat from step 1

the proxy implements automatic retry logic:

```rust
// first attempt
let response = send_request_with_dpop(request, session).await?;

if response.status() == 401 || response.status() == 400 {
  if let Some(nonce) = response.headers().get("dpop-nonce") {
    // store nonce for session
    store.update_session_dpop_nonce(did, session_id, nonce).await?;
    
    // retry with nonce
    let response = send_request_with_dpop(request, session).await?;
    return response;
  }
}
```

**ath computation**

the access token hash is computed as:

```rust
use sha2::{Sha256, Digest};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

let mut hasher = Sha256::new();
hasher.update(access_token.as_bytes());
let hash = hasher.finalize();
let ath = URL_SAFE_NO_PAD.encode(hash);
```

this ensures the dpop proof can only be used with the specific access token it was created for.

### dpop security properties

dpop provides several security guarantees:

1. **token binding**: tokens can only be used by the key holder
2. **replay protection**: each dpop proof has unique jti
3. **request binding**: proofs specify exact http method and uri
4. **temporal validity**: proofs expire after 60 seconds
5. **token coupling**: upstream proofs bound to specific access tokens via ath

these properties make token theft much less valuable - an attacker would need both the token and the private key.

## session management

the proxy maintains multiple types of session state. understanding the session lifecycle is critical.

### session structure

an oauth session in the proxy contains:

```rust
pub struct OAuthSession {
    // upstream state (proxy ↔ pds)
    pub upstream_access_token: String,
    pub upstream_refresh_token: String,
    pub upstream_dpop_jkt: String, // jkt of proxy's key
    pub upstream_expires_at: DateTime<Utc>,
    pub upstream_scope: String,
    
    // downstream state (app ↔ proxy)
    pub downstream_dpop_jkt: String, // jkt of client's key
    pub downstream_auth_code: Option<String>,
    pub downstream_refresh_token: Option<String>,
    pub downstream_expires_at: DateTime<Utc>,
    pub downstream_nonce_pad: [u8; 32], // for nonce generation
    
    // identity
    pub user_did: String,
    pub user_handle: String,
    pub pds_url: String,
    
    // session metadata
    pub session_id: String, // unique session identifier
    pub created_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    
    // dpop state
    pub dpop_nonce: Option<String>, // cached nonce for pds
    pub jti_cache: HashSet<String>, // seen jtis for replay protection
}
```

### session lifecycle states

sessions progress through several states:

```
┌─────────────────┐
│  PendingPAR     │ ← PAR request received
└────────┬────────┘
         │
         v
┌─────────────────────┐
│AwaitingAuthorization│ ← User redirected to PDS
└────────┬────────────┘
         │
         v
┌──────────────────────┐
│AwaitingTokenExchange │ ← OAuth callback received
└────────┬─────────────┘
         │
         v
┌─────────────────┐
│     Ready       │ ← Client has JWT, can make requests
└────────┬────────┘
         │
         v
┌─────────────────┐
│    Revoked      │ ← Session terminated
└─────────────────┘
```

### session storage

the proxy uses trait-based storage abstraction. sessions are stored via multiple mappings:

**primary storage**:
```
(did, session_id) → OAuthSession
```

**lookup indices**:
```
downstream_dpop_jkt → session_id
downstream_refresh_token → (did, session_id)
downstream_auth_code → (did, session_id, pkce_data)
did → session_id (active session)
```

**ephemeral storage**:
```
par_request_uri → ParData (90 second ttl)
oauth_state → PendingAuth (5 minute ttl)
```

### session lookup flows

**by jwt** (for xrpc requests):
1. validate jwt signature
2. extract `sub` claim (user did)
3. look up active session: `did → session_id`
4. retrieve full session: `(did, session_id) → OAuthSession`
5. verify dpop binding: jwt `cnf.jkt` == session `downstream_dpop_jkt`

**by refresh token** (for token refresh):
1. look up mapping: `refresh_token → (did, session_id)`
2. retrieve full session: `(did, session_id) → OAuthSession`
3. validate session is active

**by dpop jkt** (for revocation):
1. extract dpop public key from proof
2. compute jkt
3. look up mapping: `dpop_jkt → session_id`
4. retrieve full session and revoke

### session expiry and cleanup

**downstream jwt expiry**:
- jwts expire after 1 hour
- client must refresh using refresh token
- expired jwts are rejected at xrpc proxy

**upstream token expiry**:
- tokens expire after ~1 year
- `jacquard-oauth` automatically refreshes with 5-minute buffer
- refresh happens transparently during xrpc requests

**session cleanup**:
- revoked sessions should be fully deleted
- expired oauth codes/states cleaned up automatically (ttl)
- jti cache grows unbounded (consider ttl in production)

## token lifecycle

let's trace the complete lifecycle of tokens through the system.

### downstream jwt lifecycle

**phase 1: issuance**

triggered by: authorization code exchange or refresh token use

```rust
async fn issue_jwt(
    signing_key: &SigningKey, // proxy's long-lived key
    user_did: &str,
    client_dpop_jkt: &str,
    proxy_url: &str,
) -> Result<String> {
    let now = Utc::now();
    let expiry = now + Duration::hours(1);
    
    let claims = json!({
        "iss": proxy_url,
        "sub": user_did,
        "aud": proxy_url,
        "exp": expiry.timestamp(),
        "iat": now.timestamp(),
        "scope": "atproto transition:generic",
        "cnf": {
            "jkt": client_dpop_jkt // dpop binding
        }
    });
    
    let header = RegisteredHeader {
        algorithm: "ES256",
        key_id: None,
        content_type: Some("JWT"),
        ..Default::default()
    };
    
    sign_jwt(header, claims, signing_key)
}
```

**phase 2: use**

your app includes the jwt in xrpc requests:

```http
GET /xrpc/app.bsky.feed.getTimeline HTTP/1.1
Host: proxy.example.com
Authorization: DPoP eyJhbGciOiJFUzI1NiIsInR5cCI6IkpXVCJ9...
DPoP: eyJ0eXAiOiJkcG9wK2p3dCIsImFsZyI6IkVTMjU2Iiw...
```

the proxy validates:
1. jwt signature using stored public key
2. expiry timestamp (`exp` claim)
3. issuer matches proxy url
4. dpop binding: `cnf.jkt` in jwt matches jkt from dpop proof

**phase 3: expiry**

after 1 hour, the jwt expires. the proxy rejects it:

```json
{
  "error": "invalid_token",
  "error_description": "JWT has expired"
}
```

your app must refresh using the refresh token.

**phase 4: refresh**

your app exchanges refresh token for new jwt:

```http
POST /oauth/token HTTP/1.1
Content-Type: application/x-www-form-urlencoded
DPoP: ...

grant_type=refresh_token&refresh_token=...
```

the proxy issues a new jwt with fresh expiry.

### downstream refresh token lifecycle

**phase 1: issuance**

created alongside jwt during code exchange:

```rust
use rand::RngCore;

let mut refresh_token = vec![0u8; 64];
OsRng.fill_bytes(&mut refresh_token);
let refresh_token = URL_SAFE_NO_PAD.encode(&refresh_token);

store.store_refresh_token_mapping(&refresh_token, user_did, session_id).await?;
```

**phase 2: use**

client exchanges refresh token for new jwt (see phase 4 above).

**phase 3: rotation**

on each refresh, the old token is invalidated and a new one issued:

```rust
// validate old token
let (did, session_id) = store.consume_refresh_token(&old_token).await?;

// issue new jwt and refresh token
let new_jwt = issue_jwt(...).await?;
let new_refresh_token = generate_refresh_token();

store.store_refresh_token_mapping(&new_refresh_token, did, session_id).await?;
```

this prevents refresh token replay attacks.

**phase 4: revocation**

explicit revocation or session cleanup deletes refresh token:

```rust
store.delete_refresh_token_mapping(&refresh_token).await?;
```

### upstream token lifecycle

**phase 1: acquisition**

during oauth callback, after user authorizes:

```rust
let token_response = oauth_client
    .finish_authorization(auth_code, pkce_verifier)
    .await?;

let upstream_session = UpstreamSession {
    access_token: token_response.access_token,
    refresh_token: token_response.refresh_token,
    expires_at: Utc::now() + Duration::seconds(token_response.expires_in),
    dpop_jkt: token_response.dpop_jkt,
};

store.store_upstream_session(user_did, session_id, upstream_session).await?;
```

**phase 2: use**

proxy uses upstream token for xrpc forwarding:

```rust
let dpop_proof = create_dpop_proof(
    &session.upstream_dpop_key,
    "GET",
    &pds_url,
    &session.upstream_access_token,
    session.dpop_nonce.as_deref(),
).await?;

let response = client
    .get(pds_url)
    .header("Authorization", format!("DPoP {}", session.upstream_access_token))
    .header("DPoP", dpop_proof)
    .send()
    .await?;
```

**phase 3: automatic refresh**

`jacquard-oauth` handles refresh transparently:

```rust
// jacquard-oauth internally checks if token expires within 5 minutes
// if so, it automatically refreshes before making the request
let token_set = oauth_client
    .get_token_set(user_did, session_id)
    .await?; // may trigger refresh

// token_set now contains fresh access_token if it was refreshed
```

**phase 4: expiry**

after ~1 year, refresh token expires. user must re-authorize:

```rust
match oauth_client.get_token_set(user_did, session_id).await {
    Err(OAuthError::RefreshTokenExpired) => {
        // session is dead, require re-authentication
        store.delete_session(user_did, session_id).await?;
        return Err("session expired, please log in again");
    }
    Ok(token_set) => { /* proceed */ }
}
```

## xrpc proxying

xrpc proxying is where the magic happens - transparent request forwarding with dpop handling.

### xrpc proxy flow

**endpoint**: `/xrpc/{path}`

your app makes an xrpc request as if the proxy were a pds:

```http
GET /xrpc/app.bsky.feed.getTimeline?limit=50 HTTP/1.1
Host: proxy.example.com
Authorization: DPoP eyJhbGciOiJFUzI1NiIsInR5cCI6IkpXVCJ9...
DPoP: eyJ0eXAiOiJkcG9wK2p3dCIsImFsZyI6IkVTMjU2Iiw...
```

**step 1: extract and validate jwt**

```rust
async fn validate_bearer_token(
    auth_header: &str,
    dpop_header: &str,
    signing_key: &VerifyingKey,
) -> Result<JwtClaims> {
    // extract jwt from "DPoP <token>" format
    let jwt = auth_header
        .strip_prefix("DPoP ")
        .ok_or("invalid authorization header")?;
    
    // validate signature
    let claims = verify_jwt(jwt, signing_key)?;
    
    // check expiry
    if claims.exp < Utc::now().timestamp() {
        return Err("token expired");
    }
    
    // validate issuer
    if claims.iss != proxy_url {
        return Err("invalid issuer");
    }
    
    Ok(claims)
}
```

**step 2: validate dpop binding**

```rust
// validate dpop proof
let dpop_proof = parse_dpop_proof(dpop_header)?;
validate_dpop_proof(&dpop_proof, "GET", request_uri, None).await?;

// compute jkt from dpop proof
let client_jkt = compute_jkt(&dpop_proof.header.jwk)?;

// verify it matches jwt binding
if jwt_claims.cnf.jkt != client_jkt {
    return Err("dpop key mismatch");
}
```

**step 3: look up upstream session**

```rust
let user_did = &jwt_claims.sub;

// get active session id for user
let session_id = store
    .get_active_session(user_did)
    .await?
    .ok_or("no active session")?;

// load full session
let session = store
    .get_upstream_session(user_did, session_id)
    .await?
    .ok_or("session not found")?;

// verify session is active
if session.expires_at < Utc::now() {
    return Err("session expired");
}
```

**step 4: load upstream dpop key**

```rust
let dpop_key = store
    .get_session_dpop_key(user_did, session_id)
    .await?
    .ok_or("dpop key not found")?;

let signing_key = SigningKey::from_bytes(&dpop_key.private_key)?;
let verifying_key = signing_key.verifying_key();
```

**step 5: load cached dpop nonce**

```rust
let cached_nonce = store
    .get_session_dpop_nonce(user_did, session_id)
    .await?;
```

**step 6: create dpop proof for pds**

```rust
let dpop_proof = create_dpop_proof_for_pds(
    &signing_key,
    &verifying_key,
    "GET", // method
    &format!("{}/xrpc/app.bsky.feed.getTimeline", session.pds_url), // full pds url
    &session.upstream_access_token, // for ath computation
    cached_nonce.as_deref(), // optional nonce
).await?;

async fn create_dpop_proof_for_pds(
    signing_key: &SigningKey,
    verifying_key: &VerifyingKey,
    method: &str,
    uri: &str,
    access_token: &str,
    nonce: Option<&str>,
) -> Result<String> {
    // compute access token hash
    let ath = compute_ath(access_token);
    
    // create jwk from public key
    let jwk = create_jwk_from_p256_key(verifying_key)?;
    
    // generate unique jti
    let mut jti_bytes = [0u8; 16];
    OsRng.fill_bytes(&mut jti_bytes);
    let jti = URL_SAFE_NO_PAD.encode(&jti_bytes);
    
    // build header
    let header = json!({
        "typ": "dpop+jwt",
        "alg": "ES256",
        "jwk": jwk
    });
    
    // build payload
    let mut payload = json!({
        "jti": jti,
        "htm": method,
        "htu": uri,
        "iat": Utc::now().timestamp(),
        "ath": ath
    });
    
    if let Some(nonce) = nonce {
        payload["nonce"] = json!(nonce);
    }
    
    // sign and encode
    sign_jwt(header, payload, signing_key)
}
```

**step 7: forward request to pds**

```rust
let pds_url = format!("{}/xrpc/app.bsky.feed.getTimeline", session.pds_url);

let mut request = client
    .get(&pds_url)
    .header("Authorization", format!("DPoP {}", session.upstream_access_token))
    .header("DPoP", dpop_proof);

// copy query parameters
for (key, value) in original_query_params {
    request = request.query(&[(key, value)]);
}

let response = request.send().await?;
```

**step 8: handle dpop nonce requirement**

```rust
if response.status() == 401 || response.status() == 400 {
    if let Some(nonce_header) = response.headers().get("dpop-nonce") {
        let nonce = nonce_header.to_str()?;
        
        // store nonce for future requests
        store.update_session_dpop_nonce(user_did, session_id, nonce).await?;
        
        // create new dpop proof with nonce
        let new_proof = create_dpop_proof_for_pds(
            &signing_key,
            &verifying_key,
            "GET",
            &pds_url,
            &session.upstream_access_token,
            Some(nonce), // now with nonce
        ).await?;
        
        // retry request
        let response = client
            .get(&pds_url)
            .header("Authorization", format!("DPoP {}", session.upstream_access_token))
            .header("DPoP", new_proof)
            .query(&original_query_params)
            .send()
            .await?;
        
        return Ok(response);
    }
}
```

**step 9: return response to client**

```rust
// extract response body
let body = response.bytes().await?;

// copy status and headers
let mut client_response = Response::builder()
    .status(response.status());

for (key, value) in response.headers() {
    client_response = client_response.header(key, value);
}

let client_response = client_response.body(body)?;
Ok(client_response)
```

### xrpc error handling

**client errors** (4xx from pds):
- forwarded directly to client
- includes validation errors, not found, etc.

**server errors** (5xx from pds):
- forwarded directly to client
- pds is responsible for its own errors

**proxy errors**:
- invalid jwt: 401 with oauth error
- expired jwt: 401 with `invalid_token`
- missing dpop: 400 with `invalid_dpop_proof`
- dpop mismatch: 403 with `invalid_dpop_proof`
- session not found: 401 with `invalid_token`

### xrpc performance considerations

**connection pooling**:
- use `reqwest::Client` with connection pooling
- reuse tcp connections to pds
- reduces latency for repeated requests

**nonce caching**:
- store dpop nonce per session
- avoid extra round-trip for nonce on every request
- nonces typically valid for several minutes

**token refresh**:
- `jacquard-oauth` checks expiry before each request
- refreshes automatically if within 5-minute window
- minimal overhead for valid tokens

**session lookup**:
- optimize `get_active_session` lookup (indexed by did)
- cache session data in memory if possible
- reduce database queries per request

## storage abstraction

the proxy uses trait-based storage to support different backends.

### oauth session store trait

```rust
#[async_trait]
pub trait OAuthSessionStore: Send + Sync {
    // PAR data (90 second ttl)
    async fn store_par_data(
        &self,
        request_uri: &str,
        par_data: ParData,
    ) -> Result<()>;
    
    async fn consume_par_data(
        &self,
        request_uri: &str,
    ) -> Result<Option<ParData>>;
    
    // Pending authorization (5 minute ttl)
    async fn store_pending_auth(
        &self,
        state: &str,
        auth_data: PendingAuth,
    ) -> Result<()>;
    
    async fn consume_pending_auth(
        &self,
        state: &str,
    ) -> Result<Option<PendingAuth>>;
    
    // Downstream client info (10 minute ttl)
    async fn store_downstream_client_info(
        &self,
        code: &str,
        client_info: DownstreamClientInfo,
    ) -> Result<()>;
    
    async fn consume_downstream_client_info(
        &self,
        code: &str,
    ) -> Result<Option<DownstreamClientInfo>>;
    
    // Refresh token mapping
    async fn store_refresh_token_mapping(
        &self,
        refresh_token: &str,
        did: &str,
        session_id: &str,
    ) -> Result<()>;
    
    async fn get_refresh_token_mapping(
        &self,
        refresh_token: &str,
    ) -> Result<Option<(String, String)>>; // (did, session_id)
    
    async fn delete_refresh_token_mapping(
        &self,
        refresh_token: &str,
    ) -> Result<()>;
    
    // Active session lookup
    async fn store_active_session(
        &self,
        did: &str,
        session_id: &str,
    ) -> Result<()>;
    
    async fn get_active_session(
        &self,
        did: &str,
    ) -> Result<Option<String>>; // session_id
    
    async fn delete_active_session(
        &self,
        did: &str,
    ) -> Result<()>;
    
    // DPoP key storage
    async fn store_session_dpop_key(
        &self,
        did: &str,
        session_id: &str,
        jkt: &str,
        private_key: &[u8],
        public_key: &[u8],
    ) -> Result<()>;
    
    async fn get_session_dpop_key(
        &self,
        did: &str,
        session_id: &str,
    ) -> Result<Option<DpopKeyPair>>;
    
    // DPoP nonce caching
    async fn update_session_dpop_nonce(
        &self,
        did: &str,
        session_id: &str,
        nonce: &str,
    ) -> Result<()>;
    
    async fn get_session_dpop_nonce(
        &self,
        did: &str,
        session_id: &str,
    ) -> Result<Option<String>>;
    
    // JTI replay protection
    async fn check_and_consume_nonce(
        &self,
        jkt: &str,
        jti: &str,
    ) -> Result<bool>; // false if already used
    
    // Session lookup by DPoP key
    async fn get_by_dpop_jkt(
        &self,
        jkt: &str,
    ) -> Result<Option<(String, String)>>; // (did, session_id)
}
```

### key store trait

```rust
#[async_trait]
pub trait KeyStore: Send + Sync {
    // Proxy's signing key for JWT issuance
    async fn get_signing_key(&self) -> Result<SigningKey>;
    
    // Legacy: DPoP key lookup by thumbprint
    async fn get_dpop_key(&self, jkt: &str) -> Result<Option<DpopKeyPair>>;
}
```

### implementation notes

**ttl handling**:
- stores must implement ttl for ephemeral data
- par data: 90 seconds
- pending auth: 5 minutes
- downstream client info: 10 minutes

**atomicity**:
- consume operations must be atomic (get + delete)
- prevents double-use of codes/states
- use transactions if backend supports them

**indexing**:
- index by did for fast active session lookup
- index by jkt for dpop-based session lookup
- index by refresh_token for token refresh

**cleanup**:
- expired ephemeral data should be cleaned up
- jti cache may need periodic cleanup (or ttl)
- revoked sessions should be fully deleted

### example implementation (in-memory)

see `jacquard-oatproxy/examples/simple_server/memory_store.rs` for a complete in-memory implementation using `HashMap` and `RwLock`.

for production, implement using:
- **sqlite**: good for single-instance deployments
- **postgresql**: good for multi-instance deployments
- **redis**: good for high-performance caching layer

## jwt signing and validation

the proxy issues jwts for downstream clients. let's dive into the implementation.

### jwt structure

```
header.payload.signature
```

each part is base64url-encoded.

### header

```json
{
  "alg": "ES256",
  "typ": "JWT"
}
```

- `alg`: signature algorithm (p256 ecdsa)
- `typ`: token type

### payload (claims)

```json
{
  "iss": "https://proxy.example.com",
  "sub": "did:plc:user123",
  "aud": "https://proxy.example.com",
  "exp": 1700000000,
  "iat": 1699996400,
  "scope": "atproto transition:generic",
  "cnf": {
    "jkt": "base64url_sha256_thumbprint"
  }
}
```

- `iss`: issuer (proxy url)
- `sub`: subject (user did)
- `aud`: audience (proxy url)
- `exp`: expiration timestamp (unix epoch)
- `iat`: issued at timestamp
- `scope`: oauth scopes
- `cnf.jkt`: dpop key confirmation

### signing process

```rust
use jose_jws::{Signature, RegisteredHeader};
use p256::ecdsa::{SigningKey, signature::Signer};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

async fn sign_jwt(
    claims: serde_json::Value,
    signing_key: &SigningKey,
    proxy_url: &str,
) -> Result<String> {
    // create header
    let header = RegisteredHeader {
        algorithm: "ES256".to_string(),
        content_type: Some("JWT".to_string()),
        ..Default::default()
    };
    
    // serialize and encode header
    let header_json = serde_json::to_string(&header)?;
    let encoded_header = URL_SAFE_NO_PAD.encode(header_json.as_bytes());
    
    // serialize and encode claims
    let claims_json = serde_json::to_string(&claims)?;
    let encoded_claims = URL_SAFE_NO_PAD.encode(claims_json.as_bytes());
    
    // create signing input
    let signing_input = format!("{}.{}", encoded_header, encoded_claims);
    
    // sign
    let signature: p256::ecdsa::Signature = signing_key.sign(signing_input.as_bytes());
    let signature_bytes = signature.to_bytes();
    let encoded_signature = URL_SAFE_NO_PAD.encode(&signature_bytes);
    
    // combine parts
    Ok(format!("{}.{}.{}", encoded_header, encoded_claims, encoded_signature))
}
```

### validation process

```rust
use jose_jws::Signature;
use p256::ecdsa::{VerifyingKey, signature::Verifier};

async fn validate_jwt(
    jwt: &str,
    verifying_key: &VerifyingKey,
    proxy_url: &str,
) -> Result<JwtClaims> {
    // split into parts
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err("invalid jwt format");
    }
    
    let encoded_header = parts[0];
    let encoded_claims = parts[1];
    let encoded_signature = parts[2];
    
    // decode header
    let header_bytes = URL_SAFE_NO_PAD.decode(encoded_header)?;
    let header: RegisteredHeader = serde_json::from_slice(&header_bytes)?;
    
    // verify algorithm
    if header.algorithm != "ES256" {
        return Err("unsupported algorithm");
    }
    
    // decode signature
    let signature_bytes = URL_SAFE_NO_PAD.decode(encoded_signature)?;
    let signature = p256::ecdsa::Signature::from_bytes(&signature_bytes.into())?;
    
    // verify signature
    let signing_input = format!("{}.{}", encoded_header, encoded_claims);
    verifying_key.verify(signing_input.as_bytes(), &signature)?;
    
    // decode claims
    let claims_bytes = URL_SAFE_NO_PAD.decode(encoded_claims)?;
    let claims: JwtClaims = serde_json::from_slice(&claims_bytes)?;
    
    // validate claims
    if claims.iss != proxy_url {
        return Err("invalid issuer");
    }
    
    if claims.exp < Utc::now().timestamp() {
        return Err("token expired");
    }
    
    Ok(claims)
}
```

### key management

the proxy needs a long-lived p256 signing key pair.

**generation** (on first startup):

```rust
use p256::ecdsa::SigningKey;
use rand::rngs::OsRng;

let signing_key = SigningKey::random(&mut OsRng);

// serialize for storage
let private_key_bytes = signing_key.to_bytes();

// derive public key
let verifying_key = signing_key.verifying_key();
let public_key_bytes = verifying_key.to_encoded_point(false).as_bytes();
```

**storage**:

store the private key securely. options:
- database with encryption at rest
- file with restricted permissions
- hardware security module (hsm)
- key management service (kms)

**loading**:

```rust
let signing_key = SigningKey::from_bytes(&private_key_bytes)?;
```

the same key is used for all jwts issued by the proxy.

## jwk thumbprints (jkt)

jwk thumbprints (rfc 7638) are used extensively for dpop binding.

### what is a jkt?

a jkt is a sha-256 hash of the canonical representation of a public key. it's used to:
- uniquely identify keys
- bind tokens to keys (in `cnf.jkt` claim)
- look up sessions by client key

### computation process

**step 1: canonicalize the jwk**

extract only required fields in lexicographic order:

for ec (p256) keys:
```json
{"crv":"P-256","kty":"EC","x":"base64url_x","y":"base64url_y"}
```

for rsa keys:
```json
{"e":"base64url_e","kty":"RSA","n":"base64url_n"}
```

for okp (ed25519) keys:
```json
{"crv":"Ed25519","kty":"OKP","x":"base64url_x"}
```

**step 2: hash the canonical json**

```rust
use sha2::{Sha256, Digest};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

fn compute_jkt(jwk: &Jwk) -> Result<String> {
    // extract required fields
    let canonical = match jwk.kty.as_str() {
        "EC" => json!({
            "crv": jwk.crv.as_ref().ok_or("missing crv")?,
            "kty": "EC",
            "x": jwk.x.as_ref().ok_or("missing x")?,
            "y": jwk.y.as_ref().ok_or("missing y")?
        }),
        "RSA" => json!({
            "e": jwk.e.as_ref().ok_or("missing e")?,
            "kty": "RSA",
            "n": jwk.n.as_ref().ok_or("missing n")?
        }),
        "OKP" => json!({
            "crv": jwk.crv.as_ref().ok_or("missing crv")?,
            "kty": "OKP",
            "x": jwk.x.as_ref().ok_or("missing x")?
        }),
        _ => return Err("unsupported key type")
    };
    
    // serialize without whitespace
    let canonical_json = serde_json::to_string(&canonical)?;
    
    // compute sha-256
    let mut hasher = Sha256::new();
    hasher.update(canonical_json.as_bytes());
    let hash = hasher.finalize();
    
    // base64url encode
    Ok(URL_SAFE_NO_PAD.encode(&hash))
}
```

### jkt uses in the proxy

**1. dpop binding confirmation**

when issuing a jwt, include client's jkt:

```json
{
  "cnf": {
    "jkt": "computed_from_client_dpop_key"
  }
}
```

when validating xrpc requests:
- extract jkt from jwt claims
- compute jkt from dpop proof header
- verify they match

**2. session lookup**

store mapping: `jkt → session_id`

allows revocation endpoint to find session:
- extract jkt from dpop proof
- look up session by jkt
- revoke session

**3. upstream dpop binding**

pds returns `dpop_jkt` in token response, confirming the dpop binding.

### implementation example

```rust
use jose_jwk::Jwk;

async fn bind_token_to_dpop_key(
    dpop_proof: &DpopProof,
) -> Result<String> {
    // extract jwk from dpop proof header
    let jwk: Jwk = serde_json::from_value(dpop_proof.header.jwk.clone())?;
    
    // compute jkt
    let jkt = compute_jkt(&jwk)?;
    
    Ok(jkt)
}

async fn validate_dpop_binding(
    jwt_claims: &JwtClaims,
    dpop_proof: &DpopProof,
) -> Result<()> {
    // extract jkt from jwt
    let jwt_jkt = jwt_claims.cnf.jkt.as_ref()
        .ok_or("missing jkt in jwt")?;
    
    // compute jkt from dpop proof
    let dpop_jkt = bind_token_to_dpop_key(dpop_proof).await?;
    
    // verify they match
    if jwt_jkt != &dpop_jkt {
        return Err("dpop key mismatch");
    }
    
    Ok(())
}
```

## security mechanisms

let's examine the security properties of the system.

### 1. dpop binding

**threat**: token theft

tokens are bound to cryptographic keys via dpop. even if an attacker intercepts a token, they can't use it without the private key.

**protection layers**:
- jwts include `cnf.jkt` claim
- every request requires dpop proof
- dpop proof must be signed by the bound key
- jkt computed from proof must match jwt

**attack scenarios**:
- ✅ network interception: attacker sees token but can't create valid dpop proof
- ✅ malicious client: can't use another client's token without their key
- ✅ server compromise: tokens useless without client keys

### 2. jti replay protection

**threat**: dpop proof replay

an attacker could capture a valid dpop proof and replay it later.

**protection layers**:
- every dpop proof has unique `jti`
- server tracks seen jtis
- duplicate jti is rejected
- jtis scoped to session/client

**attack scenarios**:
- ✅ proof replay: server detects duplicate jti
- ✅ proof reuse: same proof can't be used twice

**implementation note**: jti cache grows unbounded. consider adding ttl or size limits in production.

### 3. request binding

**threat**: proof reuse across endpoints

an attacker could capture a dpop proof for one endpoint and reuse it for another.

**protection layers**:
- dpop proof includes `htm` (http method)
- dpop proof includes `htu` (http uri)
- server validates both match actual request
- uri comparison excludes query/fragment

**attack scenarios**:
- ✅ cross-endpoint reuse: htm/htu mismatch detected
- ✅ method confusion: GET proof can't be used for POST

### 4. temporal validity

**threat**: long-lived proof replay

an attacker could store a proof and replay it much later.

**protection layers**:
- dpop proof includes `iat` timestamp
- server rejects proofs older than 60 seconds
- nonces have 5-minute max age

**attack scenarios**:
- ✅ delayed replay: timestamp too old
- ✅ stale nonce: nonce rejected if expired

### 5. token expiry

**threat**: long-lived token theft

short-lived tokens limit damage from theft.

**protection layers**:
- downstream jwts expire after 1 hour
- upstream tokens refresh automatically
- expired tokens rejected immediately

**attack scenarios**:
- ✅ stolen jwt: expires after 1 hour max
- ✅ stolen refresh token: rotated on use

### 6. pkce (proof key for code exchange)

**threat**: authorization code interception

an attacker could intercept the authorization code during redirect.

**protection layers**:
- client generates random `code_verifier`
- par includes `code_challenge` = sha256(code_verifier)
- token exchange requires original `code_verifier`
- server validates sha256(verifier) == stored challenge

**attack scenarios**:
- ✅ code interception: attacker doesn't know verifier
- ✅ redirect manipulation: pkce validation fails

### 7. refresh token rotation

**threat**: refresh token theft and reuse

an attacker could steal and repeatedly use a refresh token.

**protection layers**:
- refresh tokens are single-use
- new refresh token issued on every use
- old refresh token invalidated
- server detects replay attempts

**attack scenarios**:
- ✅ token reuse: old token already consumed
- ✅ concurrent use: one request succeeds, others fail

### 8. nonce binding

**threat**: dpop proof pre-generation

an attacker could pre-generate dpop proofs.

**protection layers**:
- server issues unique nonces
- client must include nonce in proof
- nonce validated via hmac
- nonces time-limited (5 minutes)

**attack scenarios**:
- ✅ proof pre-generation: nonce required and validated
- ✅ nonce reuse: hmac includes timestamp

### 9. state parameter

**threat**: csrf during oauth

an attacker could trick a user into authorizing the attacker's account.

**protection layers**:
- client generates random `state`
- server echoes state in redirect
- client validates state matches

**attack scenarios**:
- ✅ csrf: state mismatch detected
- ✅ session fixation: state bound to session

### 10. upstream token isolation

**threat**: client accessing upstream token

a malicious client shouldn't see the long-lived upstream token.

**protection layers**:
- upstream tokens never sent to client
- client only receives short-lived jwts
- proxy handles all upstream communication

**attack scenarios**:
- ✅ client compromise: doesn't expose upstream session
- ✅ token extraction: upstream token never leaves proxy

### security best practices

**in production deployments**:

1. **use tls**: all communication must be over https
2. **validate origins**: check redirect uris strictly
3. **rotate keys**: periodically rotate signing keys
4. **monitor jtis**: detect unusual patterns in jti usage
5. **rate limit**: prevent brute force attacks
6. **audit logs**: log all token issuance and validation
7. **secrets management**: store keys in kms or hsm
8. **session limits**: limit concurrent sessions per user
9. **ip binding**: optionally bind sessions to ip ranges
10. **revocation api**: provide user-facing revocation ui

## implementation guide

let's walk through implementing the proxy from scratch.

### step 1: implement storage traits

create a storage backend (sqlite example):

```rust
use sqlx::{SqlitePool, Row};

pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        
        // run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await?;
        
        Ok(Self { pool })
    }
}

#[async_trait]
impl OAuthSessionStore for SqliteStore {
    async fn store_par_data(
        &self,
        request_uri: &str,
        par_data: ParData,
    ) -> Result<()> {
        let expires_at = Utc::now() + Duration::seconds(90);
        let data_json = serde_json::to_string(&par_data)?;
        
        sqlx::query(
            "INSERT INTO par_data (request_uri, data, expires_at) VALUES (?, ?, ?)"
        )
        .bind(request_uri)
        .bind(data_json)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn consume_par_data(
        &self,
        request_uri: &str,
    ) -> Result<Option<ParData>> {
        let mut tx = self.pool.begin().await?;
        
        let row = sqlx::query(
            "DELETE FROM par_data WHERE request_uri = ? AND expires_at > ? RETURNING data"
        )
        .bind(request_uri)
        .bind(Utc::now())
        .fetch_optional(&mut *tx)
        .await?;
        
        tx.commit().await?;
        
        match row {
            Some(row) => {
                let data_json: String = row.get("data");
                let par_data = serde_json::from_str(&data_json)?;
                Ok(Some(par_data))
            }
            None => Ok(None)
        }
    }
    
    // implement other methods...
}

#[async_trait]
impl KeyStore for SqliteStore {
    async fn get_signing_key(&self) -> Result<SigningKey> {
        let row = sqlx::query("SELECT private_key FROM signing_keys WHERE id = 1")
            .fetch_one(&self.pool)
            .await?;
        
        let key_bytes: Vec<u8> = row.get("private_key");
        let signing_key = SigningKey::from_bytes(&key_bytes.into())?;
        
        Ok(signing_key)
    }
    
    // implement other methods...
}
```

### step 2: create database schema

```sql
-- migrations/001_oauth_tables.sql

CREATE TABLE par_data (
    request_uri TEXT PRIMARY KEY,
    data TEXT NOT NULL,
    expires_at TIMESTAMP NOT NULL
);

CREATE INDEX idx_par_expires ON par_data(expires_at);

CREATE TABLE pending_auth (
    state TEXT PRIMARY KEY,
    data TEXT NOT NULL,
    expires_at TIMESTAMP NOT NULL
);

CREATE INDEX idx_pending_expires ON pending_auth(expires_at);

CREATE TABLE downstream_client_info (
    code TEXT PRIMARY KEY,
    data TEXT NOT NULL,
    expires_at TIMESTAMP NOT NULL
);

CREATE INDEX idx_client_info_expires ON downstream_client_info(expires_at);

CREATE TABLE refresh_tokens (
    token TEXT PRIMARY KEY,
    did TEXT NOT NULL,
    session_id TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL
);

CREATE INDEX idx_refresh_did_session ON refresh_tokens(did, session_id);

CREATE TABLE active_sessions (
    did TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    updated_at TIMESTAMP NOT NULL
);

CREATE TABLE session_dpop_keys (
    did TEXT NOT NULL,
    session_id TEXT NOT NULL,
    jkt TEXT NOT NULL,
    private_key BLOB NOT NULL,
    public_key BLOB NOT NULL,
    PRIMARY KEY (did, session_id)
);

CREATE INDEX idx_dpop_jkt ON session_dpop_keys(jkt);

CREATE TABLE session_dpop_nonces (
    did TEXT NOT NULL,
    session_id TEXT NOT NULL,
    nonce TEXT NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    PRIMARY KEY (did, session_id)
);

CREATE TABLE jti_cache (
    jkt TEXT NOT NULL,
    jti TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    PRIMARY KEY (jkt, jti)
);

CREATE INDEX idx_jti_created ON jti_cache(created_at);

CREATE TABLE signing_keys (
    id INTEGER PRIMARY KEY,
    private_key BLOB NOT NULL,
    public_key BLOB NOT NULL,
    created_at TIMESTAMP NOT NULL
);

-- generate initial signing key
-- (in rust, during first migration or startup)
```

### step 3: configure the proxy

```rust
use jacquard_oatproxy::{ProxyConfig, OAuthProxyServer};

#[tokio::main]
async fn main() -> Result<()> {
    // create storage
    let store = SqliteStore::new("sqlite://oauth.db").await?;
    
    // configure proxy
    let config = ProxyConfig {
        host: "https://proxy.example.com".to_string(),
        scope: "atproto transition:generic".to_string(),
        client_metadata: ClientMetadata {
            client_id: "https://proxy.example.com/oauth-client-metadata.json".to_string(),
            redirect_uris: vec![
                "https://proxy.example.com/oauth/return".to_string()
            ],
            grant_types: vec!["authorization_code".to_string(), "refresh_token".to_string()],
            response_types: vec!["code".to_string()],
            token_endpoint_auth_method: "private_key_jwt".to_string(),
            application_type: "web".to_string(),
            dpop_bound_access_tokens: true,
        },
        default_pds: "https://bsky.social".to_string(),
        dpop_nonce_hmac_secret: generate_secret(), // 32+ random bytes
    };
    
    // build server
    let server = OAuthProxyServer::new(config, store.clone(), store.clone());
    
    // get router
    let app = server.router();
    
    // run server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

fn generate_secret() -> Vec<u8> {
    use rand::RngCore;
    let mut secret = vec![0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut secret);
    secret
}
```

### step 4: mount the router

```rust
use axum::{Router, routing::get};

#[tokio::main]
async fn main() -> Result<()> {
    let store = SqliteStore::new("sqlite://oauth.db").await?;
    let config = create_config();
    let oauth_server = OAuthProxyServer::new(config, store.clone(), store.clone());
    
    // mount oauth routes at root
    let app = Router::new()
        .nest("/", oauth_server.router())
        .route("/health", get(health_check));
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn health_check() -> &'static str {
    "ok"
}
```

### step 5: configure clients

tell your atproto sdk to use the proxy:

**atcute example**:

```typescript
import { configureOAuth, IdentityResolver, ResolvedIdentity } from '@atcute/client';

class ProxyIdentityResolver implements IdentityResolver {
  constructor(
    private upstream: IdentityResolver,
    private proxyUrl: string,
  ) {}

  async resolve(actor: string): Promise<ResolvedIdentity> {
    const identity = await this.upstream.resolve(actor);
    return {
      ...identity,
      pds: this.proxyUrl, // rewrite pds to proxy
    };
  }
}

const proxyResolver = new ProxyIdentityResolver(
  baseResolver,
  'https://proxy.example.com',
);

configureOAuth({
  metadata: {
    client_id: 'https://myapp.example.com/client-metadata.json',
    redirect_uri: 'https://myapp.example.com/callback',
  },
  identityResolver: proxyResolver,
});
```

### step 6: implement cleanup

add periodic cleanup for expired data:

```rust
async fn cleanup_task(store: Arc<SqliteStore>) {
    let mut interval = tokio::time::interval(Duration::minutes(5));
    
    loop {
        interval.tick().await;
        
        if let Err(e) = cleanup_expired_data(&store).await {
            tracing::error!("cleanup failed: {}", e);
        }
    }
}

async fn cleanup_expired_data(store: &SqliteStore) -> Result<()> {
    let now = Utc::now();
    
    // delete expired par data
    sqlx::query("DELETE FROM par_data WHERE expires_at < ?")
        .bind(now)
        .execute(&store.pool)
        .await?;
    
    // delete expired pending auth
    sqlx::query("DELETE FROM pending_auth WHERE expires_at < ?")
        .bind(now)
        .execute(&store.pool)
        .await?;
    
    // delete expired client info
    sqlx::query("DELETE FROM downstream_client_info WHERE expires_at < ?")
        .bind(now)
        .execute(&store.pool)
        .await?;
    
    // delete old jti cache (older than 2 hours)
    let jti_cutoff = now - Duration::hours(2);
    sqlx::query("DELETE FROM jti_cache WHERE created_at < ?")
        .bind(jti_cutoff)
        .execute(&store.pool)
        .await?;
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let store = Arc::new(SqliteStore::new("sqlite://oauth.db").await?);
    
    // spawn cleanup task
    tokio::spawn(cleanup_task(store.clone()));
    
    // start server...
}
```

## configuration

the proxy is configured via `ProxyConfig`:

```rust
pub struct ProxyConfig {
    /// Public URL of the proxy (e.g., "https://proxy.example.com")
    pub host: String,
    
    /// OAuth scopes to request from PDS
    pub scope: String,
    
    /// Client metadata for upstream PDS communication
    pub client_metadata: ClientMetadata,
    
    /// Default PDS for public requests
    pub default_pds: String,
    
    /// HMAC secret for stateless DPoP nonce generation (32+ bytes)
    pub dpop_nonce_hmac_secret: Vec<u8>,
}

pub struct ClientMetadata {
    /// Client ID (URL to client metadata JSON)
    pub client_id: String,
    
    /// Allowed redirect URIs for OAuth callbacks
    pub redirect_uris: Vec<String>,
    
    /// Grant types (typically ["authorization_code", "refresh_token"])
    pub grant_types: Vec<String>,
    
    /// Response types (typically ["code"])
    pub response_types: Vec<String>,
    
    /// Token endpoint authentication method
    pub token_endpoint_auth_method: String,
    
    /// Application type ("web" or "native")
    pub application_type: String,
    
    /// Whether to use DPoP-bound access tokens
    pub dpop_bound_access_tokens: bool,
}
```

### environment-based configuration

```rust
use std::env;

fn load_config() -> Result<ProxyConfig> {
    let host = env::var("PROXY_HOST")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    
    let default_pds = env::var("DEFAULT_PDS")
        .unwrap_or_else(|_| "https://bsky.social".to_string());
    
    let secret_hex = env::var("DPOP_NONCE_SECRET")?;
    let dpop_nonce_hmac_secret = hex::decode(secret_hex)?;
    
    if dpop_nonce_hmac_secret.len() < 32 {
        return Err("DPOP_NONCE_SECRET must be at least 32 bytes");
    }
    
    Ok(ProxyConfig {
        host: host.clone(),
        scope: "atproto transition:generic".to_string(),
        client_metadata: ClientMetadata {
            client_id: format!("{}/oauth-client-metadata.json", host),
            redirect_uris: vec![format!("{}/oauth/return", host)],
            grant_types: vec![
                "authorization_code".to_string(),
                "refresh_token".to_string()
            ],
            response_types: vec!["code".to_string()],
            token_endpoint_auth_method: "private_key_jwt".to_string(),
            application_type: "web".to_string(),
            dpop_bound_access_tokens: true,
        },
        default_pds,
        dpop_nonce_hmac_secret,
    })
}
```

## metadata endpoints

the proxy exposes oauth metadata endpoints for discovery.

### oauth authorization server metadata

**endpoint**: `/.well-known/oauth-authorization-server`

returns oauth 2.1 server metadata:

```json
{
  "issuer": "https://proxy.example.com",
  "authorization_endpoint": "https://proxy.example.com/oauth/authorize",
  "token_endpoint": "https://proxy.example.com/oauth/token",
  "pushed_authorization_request_endpoint": "https://proxy.example.com/oauth/par",
  "revocation_endpoint": "https://proxy.example.com/oauth/revoke",
  "require_pushed_authorization_requests": true,
  "scopes_supported": ["atproto", "transition:generic"],
  "response_types_supported": ["code"],
  "response_modes_supported": ["query", "fragment"],
  "grant_types_supported": ["authorization_code", "refresh_token"],
  "code_challenge_methods_supported": ["S256"],
  "token_endpoint_auth_methods_supported": ["none", "private_key_jwt"],
  "dpop_signing_alg_values_supported": ["ES256", "RS256"],
  "authorization_response_iss_parameter_supported": true
}
```

### oauth protected resource metadata

**endpoint**: `/.well-known/oauth-protected-resource`

returns protected resource metadata:

```json
{
  "resource": "https://proxy.example.com",
  "authorization_servers": ["https://proxy.example.com"],
  "bearer_methods_supported": ["header"],
  "resource_signing_alg_values_supported": ["ES256"],
  "resource_documentation": "https://docs.example.com/oauth"
}
```

### client metadata

**endpoint**: `/oauth-client-metadata.json`

returns proxy's client metadata (for upstream pdses):

```json
{
  "client_id": "https://proxy.example.com/oauth-client-metadata.json",
  "client_name": "Example OAuth Proxy",
  "redirect_uris": ["https://proxy.example.com/oauth/return"],
  "grant_types": ["authorization_code", "refresh_token"],
  "response_types": ["code"],
  "token_endpoint_auth_method": "private_key_jwt",
  "application_type": "web",
  "dpop_bound_access_tokens": true
}
```

## error handling

the proxy returns oauth-compliant errors.

### oauth error format

```json
{
  "error": "error_code",
  "error_description": "human-readable description"
}
```

### error codes

**par errors**:
- `invalid_request`: missing or malformed parameters
- `invalid_dpop_proof`: dpop validation failed
- `use_dpop_nonce`: nonce required (includes `DPoP-Nonce` header)

**authorize errors**:
- `invalid_request_uri`: request_uri not found or expired
- `access_denied`: user denied authorization
- `server_error`: upstream pds error

**token errors**:
- `invalid_grant`: code not found, expired, or already used
- `invalid_request`: missing or malformed parameters
- `invalid_dpop_proof`: dpop validation failed
- `invalid_client`: client authentication failed

**xrpc errors**:
- `invalid_token`: jwt invalid, expired, or revoked
- `invalid_dpop_proof`: dpop validation failed or binding mismatch
- `unauthorized`: missing authentication

### dpop nonce errors

when a dpop nonce is required, the proxy returns:

```http
HTTP/1.1 400 Bad Request
DPoP-Nonce: base64url_encoded_nonce
Content-Type: application/json

{
  "error": "use_dpop_nonce",
  "error_description": "DPoP proof must include nonce"
}
```

the client should retry with the nonce included in the dpop proof payload.

### implementation

```rust
use axum::{
    response::{IntoResponse, Response},
    http::StatusCode,
    Json,
};

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("invalid dpop proof: {0}")]
    InvalidDpopProof(String),
    
    #[error("invalid grant")]
    InvalidGrant,
    
    #[error("invalid token")]
    InvalidToken,
    
    #[error("use dpop nonce")]
    UseDpopNonce { nonce: String },
    
    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, error_code, description) = match self {
            ProxyError::InvalidDpopProof(msg) => {
                (StatusCode::BAD_REQUEST, "invalid_dpop_proof", msg)
            }
            ProxyError::InvalidGrant => {
                (StatusCode::BAD_REQUEST, "invalid_grant", "authorization code invalid or expired".to_string())
            }
            ProxyError::InvalidToken => {
                (StatusCode::UNAUTHORIZED, "invalid_token", "token invalid or expired".to_string())
            }
            ProxyError::UseDpopNonce { nonce } => {
                return (
                    StatusCode::BAD_REQUEST,
                    [("DPoP-Nonce", nonce.as_str())],
                    Json(json!({
                        "error": "use_dpop_nonce",
                        "error_description": "DPoP proof must include nonce"
                    }))
                ).into_response();
            }
            ProxyError::Internal(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "server_error", msg)
            }
        };
        
        (
            status,
            Json(json!({
                "error": error_code,
                "error_description": description
            }))
        ).into_response()
    }
}
```

---

## conclusion

jacquard-oatproxy is a sophisticated oauth 2.1 proxy that bridges the gap between atproto's security model and practical application needs. by acting as both oauth server and client, it enables long-lived sessions while maintaining strong security through dpop binding.

key takeaways:

1. **dual oauth role**: proxy is both server (to clients) and client (to pdses)
2. **session extension**: confidential client mode extends sessions from 1 week to 1 year
3. **dpop everywhere**: proof-of-possession prevents token theft
4. **transparent proxying**: sdks work unchanged, just point to proxy url
5. **storage abstraction**: implement traits for your persistence backend
6. **security first**: multiple layers of protection (binding, replay, temporal, rotation)

for more details, see:
- `examples/simple_server/` for complete working example
- `src/` for full implementation
- atproto oauth spec: https://atproto.com/specs/oauth
- rfc 9449 (dpop): https://www.rfc-editor.org/rfc/rfc9449.html
- rfc 9126 (par): https://www.rfc-editor.org/rfc/rfc9126.html
