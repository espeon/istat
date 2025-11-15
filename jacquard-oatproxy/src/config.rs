use jacquard_oauth::atproto::AtprotoClientMetadata;
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

        Self {
            host: host.clone(),
            scope: default_scopes.clone(),
            client_metadata: AtprotoClientMetadata::new_localhost(
                Some(vec![
                    format!("{}/oauth/return", host_str)
                        .parse()
                        .expect("valid url"),
                ]),
                Some(default_scopes),
            ),
            default_pds: Url::parse("https://public.api.bsky.app").expect("valid url"),
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
}
