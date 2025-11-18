use jacquard_oauth::atproto::{AtprotoClientMetadata, GrantType};
use jacquard_oauth::scopes::Scope;
use url::Url;

/// Configuration for the OAuth proxy server
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// Public HTTPS address of this proxy
    pub host: Url,

    /// OAuth scopes to request
    pub scope: Vec<Scope<'static>>,

    /// Client metadata for OAuth flows
    pub client_metadata: AtprotoClientMetadata<'static>,

    /// Default PDS for unauthenticated/public requests
    pub default_pds: Url,

    /// HMAC secret for DPoP nonce generation (32+ bytes recommended)
    pub dpop_nonce_hmac_secret: Vec<u8>,

    /// Downstream token expiry in seconds (default: 3600 = 1 hour)
    pub downstream_token_expiry_seconds: i64,
}

impl ProxyConfig {
    /// Create a new configuration with sensible defaults
    pub fn new(host: impl Into<Url>) -> Self {
        let host = host.into();
        let host_str = host.as_str().trim_end_matches('/');

        let default_scopes = vec![
            Scope::parse("atproto").expect("valid scope"),
            Scope::parse("transition:generic").expect("valid scope"),
        ];

        // Use localhost metadata for local development, full metadata for production
        let client_metadata = if host_str.contains("localhost") || host_str.contains("127.0.0.1") {
            AtprotoClientMetadata::new_localhost(
                Some(vec![
                    format!("{}/oauth/return", host_str)
                        .parse()
                        .expect("valid url"),
                ]),
                Some(default_scopes.clone()),
            )
        } else {
            let mut metadata = AtprotoClientMetadata::new(
                format!("{}/oauth-client-metadata.json", host_str)
                    .parse()
                    .expect("valid url"),
                Some(host.clone()), // client_uri
                vec![
                    format!("{}/oauth/return", host_str)
                        .parse()
                        .expect("valid url"),
                ],
                vec![GrantType::AuthorizationCode, GrantType::RefreshToken],
                default_scopes.clone(),
                Some(
                    format!("{}/oauth/jwks.json", host_str)
                        .parse()
                        .expect("valid url"),
                ),
            );

            // Set required fields for ATProto OAuth
            metadata.client_name = Some("istat OAuth Proxy".into());
            metadata.tos_uri = Some(format!("{}/tos", host_str).parse().expect("valid url"));
            metadata.logo_uri = Some(format!("{}/logo.png", host_str).parse().expect("valid url"));
            metadata.scopes = default_scopes.clone();

            metadata
        };

        Self {
            host: host.clone(),
            scope: default_scopes.clone(),
            client_metadata,
            default_pds: Url::parse("https://public.api.bsky.app").expect("valid url"),
            dpop_nonce_hmac_secret: b"insecure-default-dpop-nonce-secret".to_vec(),
            downstream_token_expiry_seconds: 3600, // 1 hour default
        }
    }

    /// Set custom scopes
    pub fn with_scopes(mut self, scopes: Vec<Scope<'static>>) -> Self {
        self.scope = scopes;
        self
    }

    /// Set default PDS
    pub fn with_default_pds(mut self, pds: Url) -> Self {
        self.default_pds = pds;
        self
    }

    /// Set HMAC secret for DPoP nonce generation
    pub fn with_dpop_nonce_secret(mut self, secret: Vec<u8>) -> Self {
        self.dpop_nonce_hmac_secret = secret;
        self
    }

    /// Set downstream token expiry in seconds
    pub fn with_downstream_token_expiry(mut self, seconds: i64) -> Self {
        self.downstream_token_expiry_seconds = seconds;
        self
    }

    /// Set client name
    pub fn with_client_name(mut self, name: impl Into<String>) -> Self {
        self.client_metadata.client_name = Some(name.into().into());
        self
    }

    /// Set ToS URI
    pub fn with_tos_uri(mut self, uri: Url) -> Self {
        self.client_metadata.tos_uri = Some(uri);
        self
    }

    /// Set logo URI
    pub fn with_logo_uri(mut self, uri: Url) -> Self {
        self.client_metadata.logo_uri = Some(uri);
        self
    }

    /// Set client URI
    pub fn with_client_uri(mut self, uri: Url) -> Self {
        self.client_metadata.client_uri = Some(uri);
        self
    }

    /// Set redirect URIs
    pub fn with_redirect_uris(mut self, uris: Vec<Url>) -> Self {
        self.client_metadata.redirect_uris = uris;
        self
    }

    /// Set policy URI
    pub fn with_policy_uri(mut self, uri: Url) -> Self {
        self.client_metadata.privacy_policy_uri = Some(uri);
        self
    }

    /// Generate a new P256 signing key for this instance
    pub fn generate_signing_key() -> p256::ecdsa::SigningKey {
        use rand::rngs::OsRng;
        p256::ecdsa::SigningKey::random(&mut OsRng)
    }

    /// Get JWKS document for a signing key
    pub fn signing_key_to_jwks(signing_key: &p256::ecdsa::SigningKey) -> serde_json::Value {
        use base64::Engine;

        let verifying_key = signing_key.verifying_key();
        let encoded_point = verifying_key.to_encoded_point(false);

        let x = encoded_point.x().expect("valid x coordinate");
        let y = encoded_point.y().expect("valid y coordinate");

        serde_json::json!({
            "keys": [{
                "kty": "EC",
                "crv": "P-256",
                "x": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(x.iter().as_slice()),
                "y": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(y.iter().as_slice()),
                "use": "sig",
                "alg": "ES256",
                "kid": "proxy-signing-key"
            }]
        })
    }
}
