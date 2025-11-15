//! # jacquard-oatproxy
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
//! use jacquard_oatproxy::{OAuthProxyServer, ProxyConfig};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = ProxyConfig::new("https://myproxy.example.com".parse()?);
//! let proxy = OAuthProxyServer::builder()
//!     .config(config)
//!     .session_store(my_session_store)
//!     .key_store(my_key_store)
//!     .nonce_store(my_nonce_store)
//!     .build()?;
//!
//! let app = proxy.router();
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod error;
pub mod server;
pub mod session;
pub mod store;
pub mod token;

pub use config::ProxyConfig;
pub use error::{Error, Result};
pub use server::{OAuthProxyServer, OAuthProxyServerBuilder};
pub use session::{OAuthSession, SessionState};
pub use store::{KeyStore, NonceStore, OAuthSessionStore};
