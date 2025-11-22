use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::header::COOKIE,
    http::{Request, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use jacquard::api::com_atproto::identity::resolve_handle::ResolveHandleRequest;
use jacquard_axum::IntoRouter;
use lexicons::vg_nat::istat::{
    actor::get_profile::GetProfileRequest,
    moji::search_emoji::SearchEmojiRequest,
    status::{
        get_status::GetStatusRequest, list_statuses::ListStatusesRequest,
        list_user_statuses::ListUserStatusesRequest,
    },
};
use miette::{IntoDiagnostic, Result};
use serde::Serialize;
use sqlx::{Row, sqlite::SqlitePool};
use std::path::PathBuf;
use tower::ServiceExt;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

mod jetstream;
mod oatproxy;
mod xrpc;

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    public_url: String,
    key_store: oatproxy::SqliteStore,
}

#[derive(Serialize)]
struct ClientMetadata {
    client_id: String,
    client_name: String,
    client_uri: String,
    redirect_uris: Vec<String>,
    scope: String,
    grant_types: Vec<String>,
    response_types: Vec<String>,
    application_type: String,
    token_endpoint_auth_method: String,
    dpop_bound_access_tokens: bool,
}

async fn handle_client_metadata(State(state): State<AppState>) -> Json<ClientMetadata> {
    let base_url = &state.public_url;
    Json(ClientMetadata {
        client_id: format!("{}/client-metadata.json", base_url),
        client_name: "iStat Web Client".to_string(),
        client_uri: base_url.clone(),
        redirect_uris: vec![format!("{}/oauth/callback", base_url)],
        scope: "atproto transition:generic".to_string(),
        grant_types: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ],
        response_types: vec!["code".to_string()],
        application_type: "web".to_string(),
        token_endpoint_auth_method: "none".to_string(),
        dpop_bound_access_tokens: true,
    })
}

// async fn handle_root(State(state): State<AppState>, headers: axum::http::HeaderMap) -> String {
//     // Try to get session from cookie
//     let session_id = headers
//         .get(COOKIE)
//         .and_then(|v| v.to_str().ok())
//         .and_then(|cookies| {
//             cookies.split(';').find_map(|cookie| {
//                 let cookie = cookie.trim();
//                 cookie.strip_prefix("session_id=")
//             })
//         });

//     if let Some(session_id) = session_id {
//         // Look up session in database
//         if let Ok(Some(row)) = sqlx::query(
//             r#"
//             SELECT did
//             FROM auth_sessions
//             WHERE id = ? AND expires_at > datetime('now')
//             "#,
//         )
//         .bind(session_id)
//         .fetch_optional(&state.db)
//         .await
//         {
//             let did: String = row.get(0);
//             return format!("hello, {}!", did);
//         }
//     }

//     "hello world!".to_string()
// }

async fn init_db(db_url: &str) -> Result<SqlitePool> {
    let pool = SqlitePool::connect(db_url).await.into_diagnostic()?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .into_diagnostic()?;

    Ok(pool)
}

#[tokio::main]
async fn main() -> Result<()> {
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

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:istat.db".to_string());
    let public_url =
        std::env::var("PUBLIC_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());

    tracing::info!("Public URL: {}", public_url);
    tracing::info!("Bind address: {}", bind_addr);

    let pool = init_db(&db_url).await?;

    let jetstream_pool = pool.clone();
    tokio::spawn(async move {
        if let Err(e) = jetstream::start_jetstream(jetstream_pool).await {
            eprintln!("Jetstream error: {}", e);
        }
    });

    // Set up OAuth proxy
    // Load or generate signing key
    let signing_key = match sqlx::query("SELECT private_key FROM oatproxy_signing_key WHERE id = 1")
        .fetch_optional(&pool)
        .await
        .into_diagnostic()?
    {
        Some(row) => {
            let key_bytes: Vec<u8> = row.try_get("private_key").into_diagnostic()?;
            let key_array: [u8; 32] = key_bytes
                .try_into()
                .map_err(|_| miette::miette!("invalid signing key length"))?;
            Some(p256::ecdsa::SigningKey::from_bytes(&key_array.into()).into_diagnostic()?)
        }
        None => {
            let signing_key = p256::ecdsa::SigningKey::random(&mut rand::rngs::OsRng);
            let key_bytes = signing_key.to_bytes();

            sqlx::query("INSERT INTO oatproxy_signing_key (id, private_key) VALUES (1, ?)")
                .bind(&key_bytes[..])
                .execute(&pool)
                .await
                .into_diagnostic()?;

            Some(signing_key)
        }
    };

    // Load or generate HMAC secret for DPoP nonces
    let hmac_secret =
        match sqlx::query("SELECT hmac_secret FROM oatproxy_dpop_hmac_secret WHERE id = 1")
            .fetch_optional(&pool)
            .await
            .into_diagnostic()?
        {
            Some(row) => row.try_get::<Vec<u8>, _>("hmac_secret").into_diagnostic()?,
            None => {
                let mut secret = vec![0u8; 32];
                rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut secret);

                sqlx::query(
                    "INSERT INTO oatproxy_dpop_hmac_secret (id, hmac_secret) VALUES (1, ?)",
                )
                .bind(&secret)
                .execute(&pool)
                .await
                .into_diagnostic()?;

                secret
            }
        };

    let mut store_builder = oatproxy::SqliteStore::builder(pool.clone());
    if let Some(key) = signing_key {
        store_builder = store_builder.with_signing_key(key);
    }
    let oatproxy_store = store_builder.build();

    // Build proxy config with optional customization
    let mut proxy_config =
        jacquard_oatproxy::ProxyConfig::new(url::Url::parse(&public_url).into_diagnostic()?)
            .with_dpop_nonce_secret(hmac_secret);

    // Configure upstream client metadata via env vars
    if let Ok(client_name) = std::env::var("ISTAT_CLIENT_NAME") {
        proxy_config = proxy_config.with_client_name(client_name);
    }

    if let Ok(tos_uri) = std::env::var("ISTAT_TOS_URI") {
        if let Ok(uri) = url::Url::parse(&tos_uri) {
            proxy_config = proxy_config.with_tos_uri(uri);
        }
    }

    if let Ok(logo_uri) = std::env::var("ISTAT_LOGO_URI") {
        if let Ok(uri) = url::Url::parse(&logo_uri) {
            proxy_config = proxy_config.with_logo_uri(uri);
        }
    }

    if let Ok(policy_uri) = std::env::var("ISTAT_POLICY_URI") {
        if let Ok(uri) = url::Url::parse(&policy_uri) {
            proxy_config = proxy_config.with_policy_uri(uri);
        }
    }

    let oatproxy_server = jacquard_oatproxy::OAuthProxyServer::builder()
        .config(proxy_config)
        .session_store(oatproxy_store.clone())
        .key_store(oatproxy_store.clone())
        .build()
        .into_diagnostic()?;

    let state = AppState {
        db: pool,
        public_url: public_url.clone(),
        key_store: oatproxy_store.clone(),
    };

    let xrpc_router = Router::new()
        .route(
            "/client-metadata.json",
            axum::routing::get(handle_client_metadata),
        )
        .merge(ResolveHandleRequest::into_router(xrpc::handle_resolve))
        .merge(GetProfileRequest::into_router(xrpc::handle_get_profile))
        .merge(SearchEmojiRequest::into_router(xrpc::handle_search_emoji))
        .merge(GetStatusRequest::into_router(xrpc::handle_get_status))
        .merge(ListUserStatusesRequest::into_router(
            xrpc::handle_list_user_statuses,
        ))
        .merge(ListStatusesRequest::into_router(xrpc::handle_list_statuses))
        // Moderation endpoints
        .route(
            "/xrpc/vg.nat.istat.moderation.blacklistCid",
            axum::routing::post(xrpc::moderation::handle_blacklist_cid),
        )
        .route(
            "/xrpc/vg.nat.istat.moderation.removeBlacklist",
            axum::routing::post(xrpc::moderation::handle_remove_blacklist),
        )
        .route(
            "/xrpc/vg.nat.istat.moderation.listBlacklisted",
            axum::routing::get(xrpc::moderation::handle_list_blacklisted),
        )
        .route(
            "/xrpc/vg.nat.istat.moderation.isAdmin",
            axum::routing::get(xrpc::moderation::handle_is_admin),
        )
        .route(
            "/xrpc/vg.nat.istat.moji.deleteEmoji",
            axum::routing::post(xrpc::moderation::handle_delete_emoji),
        )
        .route(
            "/xrpc/vg.nat.istat.status.deleteStatus",
            axum::routing::post(xrpc::moderation::handle_delete_status),
        )
        .with_state(state.clone());

    let dev_mode = std::env::var("DEV_MODE").unwrap_or_default() == "true";
    let disable_frontend = std::env::var("ISTAT_DISABLE_FRONTEND").unwrap_or_default() == "true";

    let app = if disable_frontend {
        // Frontend disabled - only serve API and OAuth endpoints
        tracing::info!("Frontend disabled - serving only API and OAuth endpoints");

        Router::new()
            .merge(xrpc_router)
            .with_state(state.clone())
            .fallback_service(oatproxy_server.router())
            .layer(CorsLayer::permissive())
    } else if dev_mode {
        // In dev mode, proxy non-API requests to Vite dev server
        tracing::info!("Running in dev mode - proxying to Vite at localhost:3001");

        let client = reqwest::Client::new();

        // Create a service that tries oatproxy, then vite
        let vite_proxy = move |req: axum::http::Request<axum::body::Body>| {
            let client = client.clone();
            async move {
                let uri = req.uri();
                let path_and_query = uri
                    .path_and_query()
                    .map(|pq| pq.as_str())
                    .unwrap_or(uri.path());

                // Proxy to Vite dev server
                let proxy_url = format!("http://localhost:3001{}", path_and_query);

                match client
                    .request(req.method().clone(), &proxy_url)
                    .send()
                    .await
                {
                    Ok(response) => {
                        let status = response.status();
                        let mut builder = axum::http::Response::builder().status(status);

                        // Copy relevant headers
                        for (key, value) in response.headers() {
                            if key != "content-length" && key != "transfer-encoding" {
                                builder = builder.header(key, value);
                            }
                        }

                        let body = response.bytes().await.unwrap_or_default();
                        builder.body(axum::body::Body::from(body)).unwrap()
                    }
                    Err(e) => {
                        tracing::error!("Proxy error for {}: {}", proxy_url, e);
                        axum::http::Response::builder()
                            .status(502)
                            .body(axum::body::Body::from("Bad Gateway"))
                            .unwrap()
                    }
                }
            }
        };

        Router::new()
            .merge(xrpc_router)
            .with_state(state.clone())
            .fallback_service(oatproxy_server.router().fallback(vite_proxy))
            .layer(CorsLayer::permissive())
    } else {
        // In prod mode, serve static files from dist directory (SPA mode)
        tracing::info!("Running in production mode - serving static files from dist/ (SPA mode)");

        let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "dist".to_string());
        let static_dir_clone = static_dir.clone();

        // Create a custom service for SPA fallback
        let spa_fallback = move |req: Request<Body>| {
            let static_dir = static_dir_clone.clone();
            async move {
                let path = req.uri().path();
                let file_path = PathBuf::from(&static_dir).join(path.trim_start_matches('/'));

                // If file exists, serve it; otherwise serve index.html for client-side routing
                if file_path.is_file() {
                    ServeDir::new(&static_dir).oneshot(req).await
                } else {
                    // Serve index.html for SPA routing
                    //let index_path = PathBuf::from(&static_dir).join("index.html");
                    ServeDir::new(&static_dir)
                        .oneshot(
                            Request::builder()
                                .uri("/index.html")
                                .body(Body::empty())
                                .unwrap(),
                        )
                        .await
                }
            }
        };

        Router::new()
            .merge(xrpc_router)
            .with_state(state.clone())
            .fallback_service(oatproxy_server.router().fallback(spa_fallback))
            .layer(CorsLayer::permissive())
    };

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .into_diagnostic()?;
    tracing::info!(
        "Server listening on {}",
        listener.local_addr().into_diagnostic()?
    );

    axum::serve(listener, app).await.unwrap();
    Ok(())
}
