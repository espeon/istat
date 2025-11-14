//! # jacquard-oauth-proxy
//!
//! A transparent ATProto OAuth proxy that handles authentication server-side.
//!
//! This crate implements a full OAuth 2.1 authorization server that sits between
//! ATProto clients and Personal Data Servers (PDS). It allows clients to use
//! standard ATProto SDKs without managing tokens, DPoP keys, or token refresh.
//!
//! ## Features
//!
//! - **Full OAuth 2.1 Server**: Implements PAR, authorize, token, and revoke endpoints
//! - **DPoP Support**: Handles DPoP proof generation and validation
//! - **Token Management**: Automatic upstream token refresh
//! - **Replay Protection**: Nonce management and JTI caching
//! - **Pluggable Storage**: Abstract traits for sessions, keys, and nonces
//!
//! ## Example
//!
//! ```rust,no_run
//! use jacquard_oauth_proxy::prelude::*;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let proxy = OAuthProxyServer::builder("https://myproxy.example.com")
//!     .session_store(my_session_store)
//!     .key_store(my_key_store)
//!     .nonce_store(my_nonce_store)
//!     .build()?;
//!
//! let app = axum::Router::new()
//!     .merge(proxy.into_router());
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod error;
pub mod server;
pub mod session;
pub mod store;
pub mod token;

pub mod prelude {
    pub use crate::config::ProxyConfig;
    pub use crate::error::{Error, Result};
    pub use crate::server::{OAuthProxyServer, OAuthProxyServerBuilder};
    pub use crate::session::{OAuthSession, SessionState};
    pub use crate::store::{KeyStore, NonceStore, OAuthSessionStore};
}
