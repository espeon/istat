## jacquard-oatproxy

> or: oatproxy but written in rust

jacquard-oatproxy implements a relatively decent OAuth 2.1 authorization server
that sits between ATProto clients and Personal Data Servers (PDSes). 

It allows developers to use standard ATProto SDKs via a more ""secure"" confidential 
client mode and optionally can be used in a BFF (Backend for Frontend) architecture if 
needed. This confidential client mode unlocks longer session times, up from 1 week 
to about a year.

## How this works

The proxy acts as a middleman between your app and the user's PDS. Basically:

1. **Your app starts oauth flow** → proxy intercepts the request
2. **Proxy authenticates with PDS** → uses its own confidential client credentials to get a long-lived session with the user's PDS
3. **Proxy issues tokens to your app** → gives your app short-lived JWT access tokens (1 hour)
4. **Your app makes xrpc requests** → sends requests through the proxy with its JWT
5. **Proxy validates and forwards** → checks the JWT, looks up the upstream session, and forwards the request to the real PDS with proper DPoP authentication

From the PDS's perspective, the proxy is the client. From your app's perspective, the proxy looks like a PDS. This lets you use confidential client mode (longer sessions) while still working with standard ATProto SDKs that expect to talk directly to a PDS.

More completely:
```mermaid
sequenceDiagram
    participant App as Your App
    participant Proxy as jacquard-oatproxy
    participant PDS as User's PDS

    Note over App,PDS: OAuth Flow (Confidential Client)
    App->>Proxy: POST /oauth/par<br/>(client_id, redirect_uri, scope)
    Proxy->>Proxy: Generate proxy state
    Proxy->>PDS: Upstream OAuth PAR<br/>(confidential client creds)
    PDS-->>Proxy: request_uri
    Proxy-->>App: request_uri (proxied)
    
    App->>Proxy: GET /oauth/authorize?request_uri=...
    Proxy->>PDS: Redirect to upstream authorize
    Note over PDS: User logs in
    PDS->>Proxy: GET /oauth/return?code=...
    Proxy->>PDS: POST /oauth/token<br/>(exchange code + DPoP proof)
    PDS-->>Proxy: access_token + refresh_token<br/>(1 year session)
    Proxy->>Proxy: Store upstream session & DPoP key
    Proxy->>Proxy: Generate downstream JWT (1h)
    Proxy-->>App: Redirect with code
    
    App->>Proxy: POST /oauth/token<br/>(exchange code)
    Proxy->>Proxy: Validate code, issue JWT
    Proxy-->>App: JWT access_token (1 hour)<br/>+ refresh_token

    Note over App,PDS: XRPC Proxying
    App->>Proxy: GET /xrpc/app.bsky.feed.getTimeline<br/>Authorization: Bearer <jwt>
    Proxy->>Proxy: Validate JWT signature
    Proxy->>Proxy: Extract DID from JWT claims
    Proxy->>Proxy: Lookup upstream session by DID
    Proxy->>Proxy: Load upstream DPoP key
    Proxy->>Proxy: Create DPoP proof for PDS
    Proxy->>PDS: GET /xrpc/app.bsky.feed.getTimeline<br/>Authorization: DPoP <upstream_token><br/>DPoP: <proof>
    
    alt DPoP nonce required
        PDS-->>Proxy: 401 + DPoP-Nonce header
        Proxy->>Proxy: Store nonce, retry with new proof
        Proxy->>PDS: Retry with nonce in DPoP proof
    end
    
    PDS-->>Proxy: 200 OK (response data)
    Proxy-->>App: 200 OK (proxied response)

    Note over App,PDS: Token Refresh
    App->>Proxy: POST /oauth/token<br/>(grant_type=refresh_token)
    Proxy->>Proxy: Validate refresh token
    Proxy->>Proxy: Check upstream session still valid
    Proxy->>Proxy: Issue new JWT (1 hour)
    Proxy-->>App: New JWT access_token
```

## Usage
You'll need to tell lies to the ATProto SDKs about where your PDS is located :) 
Here are a few ways to do that with some common libraries.

### with atcute
You will need a custom private identity resolver. Here's one.
```ts
/**
 * Wraps an existing identity resolver and rewrites PDS endpoints to point to our OAuth proxy
 */
export class ProxyIdentityResolver implements IdentityResolver {
  constructor(
    private upstream: IdentityResolver,
    private proxyUrl: string,
  ) {}

  async resolve(
    actor: ActorIdentifier,
    options?: ResolveIdentityOptions,
  ): Promise<ResolvedIdentity> {
    // use the upstream resolver to get the actual identity
    const identity = await this.upstream.resolve(actor, options);

    // rewrite the PDS endpoint to point to our proxy
    console.log(
      "Rewriting PDS endpoint from",
      identity.pds,
      "to",
      this.proxyUrl,
    );

    return {
      ...identity,
      pds: this.proxyUrl,
    };
  }
}
```

Use it like so:

```ts
  const proxyResolver = new ProxyIdentityResolver(
    baseResolver,
    OAUTH_PROXY_URL,
  );

  configureOAuth({
    metadata: {
      client_id,
      redirect_uri,
    },
    identityResolver: proxyResolver,
  });
```

### with @atproto/oauth-client-browser (and similar)
You'll need to overwrite the default fetch handler with one that changes the PDS URL to point to the proxy.
```ts
import { BrowserOAuthClient, OAuthClient } from "@atproto/oauth-client-browser";

const fetchWithLies = async (
  oatProxyUrl: string,
  input: RequestInfo | URL,
  init?: RequestInit
) => {
  // Normalize input to a Request object
  let request: Request;
  if (typeof input === "string" || input instanceof URL) {
    request = new Request(input, init);
  } else {
    request = input;
  }

  if (
    request.url.includes("plc.directory") || // did:plc
    request.url.endsWith("did.json") // did:web
  ) {
    const res = await fetch(request, init);
    if (!res.ok) {
      return res;
    }
    const data = await res.json();
    const service = data.service.find((s: any) => s.id === "#atproto_pds");
    if (!service) {
      return res;
    }
    service.serviceEndpoint = oatProxyUrl;
    return new Response(JSON.stringify(data), {
      status: res.status,
      headers: res.headers,
    });
  }

  return fetch(request, init);
};

export default async function createOAuthClient(
  oatProxyUrl: string
): Promise<OAuthClient> {
  return await BrowserOAuthClient.load({
    clientId: `${oatProxyUrl}/oauth/downstream/client-metadata.json`,
    handleResolver: oatProxyUrl,
    responseMode: "query",

    // Lie to the oauth client and use our upstream server instead
    fetch: (input, init) => fetchWithLies(oatProxyUrl, input, init),
  });
}```
