//! Framework-agnostic OAuth handler functions.
//!
//! Re-exports the server types for now. In the future, this module could provide
//! framework-agnostic handler functions that can be wrapped by different web frameworks.

// Re-export the server types
pub use crate::server::{OAuthProxyServer, OAuthProxyServerBuilder};
