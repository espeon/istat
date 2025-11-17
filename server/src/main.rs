use axum::{Router, extract::State, http::header::COOKIE};
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
use sqlx::{Row, sqlite::SqlitePool};
use tower_http::cors::CorsLayer;

mod jetstream;
mod oatproxy;
mod xrpc;

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
}

async fn handle_root(State(state): State<AppState>, headers: axum::http::HeaderMap) -> String {
    // Try to get session from cookie
    let session_id = headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let cookie = cookie.trim();
                cookie.strip_prefix("session_id=")
            })
        });

    if let Some(session_id) = session_id {
        // Look up session in database
        if let Ok(Some(row)) = sqlx::query(
            r#"
            SELECT did
            FROM auth_sessions
            WHERE id = ? AND expires_at > datetime('now')
            "#,
        )
        .bind(session_id)
        .fetch_optional(&state.db)
        .await
        {
            let did: String = row.get(0);
            return format!("hello, {}!", did);
        }
    }

    "hello world!".to_string()
}

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
    let proxy_config =
        jacquard_oatproxy::ProxyConfig::new(url::Url::parse(&public_url).into_diagnostic()?)
            .with_dpop_nonce_secret(hmac_secret);

    let oatproxy_server = jacquard_oatproxy::OAuthProxyServer::builder()
        .config(proxy_config)
        .session_store(oatproxy_store.clone())
        .key_store(oatproxy_store.clone())
        .build()
        .into_diagnostic()?;

    let state = AppState { db: pool };

    let xrpc_router = Router::new()
        .merge(ResolveHandleRequest::into_router(xrpc::handle_resolve))
        .merge(GetProfileRequest::into_router(xrpc::handle_get_profile))
        .merge(SearchEmojiRequest::into_router(xrpc::handle_search_emoji))
        .merge(GetStatusRequest::into_router(xrpc::handle_get_status))
        .merge(ListUserStatusesRequest::into_router(
            xrpc::handle_list_user_statuses,
        ))
        .merge(ListStatusesRequest::into_router(xrpc::handle_list_statuses))
        .with_state(state.clone());

    let dev_mode = std::env::var("DEV_MODE").unwrap_or_default() == "true";

    let app = if dev_mode {
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
        // In prod mode, use OAuth proxy as fallback
        Router::new()
            .route("/", axum::routing::get(handle_root))
            .merge(xrpc_router)
            .with_state(state)
            .fallback_service(oatproxy_server.router())
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
