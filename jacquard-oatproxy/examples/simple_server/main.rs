//! Simple example OAuth proxy server using in-memory storage.
//!
//! This example demonstrates how to set up a basic OAuth proxy server
//! that can handle ATProto authentication flows.
//!
//! Run with:
//! ```
//! cargo run --example simple_server
//! ```

mod memory_store;

use jacquard_oatproxy::{OAuthProxyServer, ProxyConfig};
use memory_store::MemoryStore;
use miette::{Context, IntoDiagnostic};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> miette::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::filter::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "simple_server=debug,jacquard_oauth_proxy=debug,info"
                    .parse()
                    .unwrap()
            }),
        )
        .init();

    // Create in-memory storage
    let store = Arc::new(MemoryStore::new());

    // Configure the proxy
    let config = ProxyConfig::new(url::Url::parse("http://127.0.0.1:3000").unwrap());

    // Build the OAuth proxy server
    let proxy = OAuthProxyServer::builder()
        .config(config)
        .session_store(store.clone())
        .key_store(store.clone())
        .nonce_store(store)
        .build()
        .into_diagnostic()
        .wrap_err("failed to build OAuth proxy server")?;

    // Create the axum app with all OAuth endpoints and CORS
    let app = proxy.router().layer(CorsLayer::permissive());

    // Start the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("OAuth proxy server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .into_diagnostic()
        .wrap_err("failed to bind to address")?;

    axum::serve(listener, app)
        .await
        .into_diagnostic()
        .wrap_err("server error")?;

    Ok(())
}
