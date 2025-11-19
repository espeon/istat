import {
  configureOAuth,
  defaultIdentityResolver,
} from "@atcute/oauth-browser-client";

import {
  CompositeDidDocumentResolver,
  PlcDidDocumentResolver,
  WebDidDocumentResolver,
  XrpcHandleResolver,
} from "@atcute/identity-resolver";

import { ProxyIdentityResolver } from "./proxy-resolver";

let isConfigured = false;

// OAuth proxy URL - defaults to current origin, can be overridden via env var
const OAUTH_PROXY_URL =
  import.meta.env.VITE_OAUTH_PROXY_URL || window.location.origin;

export function initOAuth() {
  if (isConfigured) {
    console.log("OAuth already configured, skipping");
    return;
  }

  // Default to current origin for client_id and redirect_uri if not specified
  const origin = window.location.origin;
  const client_id =
    import.meta.env.VITE_OAUTH_CLIENT_ID || `${origin}/client-metadata.json`;
  const redirect_uri =
    import.meta.env.VITE_OAUTH_REDIRECT_URI || `${origin}/oauth/callback`;

  console.log("Configuring OAuth with:", {
    client_id,
    redirect_uri,
    proxy: OAUTH_PROXY_URL,
  });

  // Create the base identity resolver
  const baseResolver = defaultIdentityResolver({
    handleResolver: new XrpcHandleResolver({
      serviceUrl: "https://public.api.bsky.app",
    }),

    didDocumentResolver: new CompositeDidDocumentResolver({
      methods: {
        plc: new PlcDidDocumentResolver(),
        web: new WebDidDocumentResolver(),
      },
    }),
  });

  // Wrap it with our proxy resolver to rewrite PDS endpoints
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

  isConfigured = true;
  console.log("OAuth configured with proxy resolver");
}
