use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{Method, StatusCode, Uri, header::COOKIE},
    response::Response,
};
use jacquard::api::com_atproto::identity::resolve_handle::{
    ResolveHandleOutput, ResolveHandleRequest,
};
use jacquard_axum::{ExtractXrpc, IntoRouter};
use jacquard_common::types::string::Did;
use lexicons::vg_nat::istat::status::{
    get_status::{GetStatusOutput, GetStatusRequest},
    list_statuses::{ListStatusesOutput, ListStatusesRequest},
    list_user_statuses::{ListUserStatusesOutput, ListUserStatusesRequest},
};
use miette::{IntoDiagnostic, Result};
use sqlx::{Row, sqlite::SqlitePool};
use std::{collections::BTreeMap, str::FromStr};

mod jetstream;
mod oauth;

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    auth_store: oauth::SharedAuthStore,
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

async fn proxy_to_pds(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    method: Method,
    uri: Uri,
    body: Body,
) -> Result<Response, StatusCode> {
    // Extract session to get the user's PDS
    let session_id = headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let cookie = cookie.trim();
                cookie.strip_prefix("session_id=")
            })
        });

    let pds_url = if let Some(session_id) = session_id {
        // Look up user's PDS from session
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
            let _did: String = row.get(0);
            // Resolve DID to PDS - for now just use bsky.social
            // TODO: proper DID resolution
            format!("https://bsky.social")
        } else {
            // Default to public API
            "https://public.api.bsky.app".to_string()
        }
    } else {
        // No session, use public API
        "https://public.api.bsky.app".to_string()
    };

    // Build the proxied request
    let path_and_query = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(uri.path());

    let url = format!("{}{}", pds_url, path_and_query);

    let client = reqwest::Client::new();
    let mut req_builder = client.request(method.clone(), &url);

    // Forward relevant headers
    for (name, value) in headers.iter() {
        if name != "host" && name != "connection" {
            if let Ok(value_str) = value.to_str() {
                req_builder = req_builder.header(name.as_str(), value_str);
            }
        }
    }

    // Forward body
    let body_bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !body_bytes.is_empty() {
        req_builder = req_builder.body(body_bytes.to_vec());
    }

    // Execute the request
    let resp = req_builder
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    // Build response
    let status = resp.status();
    let headers = resp.headers().clone();
    let body_bytes = resp.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    let mut response = Response::builder().status(status);

    // Copy response headers
    for (name, value) in headers.iter() {
        response = response.header(name, value);
    }

    response
        .body(Body::from(body_bytes))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn handle_resolve(
    ExtractXrpc(req): ExtractXrpc<ResolveHandleRequest>,
) -> Result<Json<ResolveHandleOutput<'static>>, StatusCode> {
    let handle = req.handle;
    let url = format!(
        "https://public.api.bsky.app/xrpc/com.atproto.identity.resolveHandle?handle={}",
        handle
    );
    let resp = reqwest::get(&url)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !resp.status().is_success() {
        return Err(StatusCode::NOT_FOUND);
    }
    let resp_json: BTreeMap<String, String> = resp
        .json()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let did_str = resp_json.get("did").ok_or(StatusCode::NOT_FOUND)?;
    let did = Did::from_str(did_str).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let output = ResolveHandleOutput {
        did,
        extra_data: None,
    };

    Ok(Json(output))
}

async fn handle_get_status(
    State(state): State<AppState>,
    ExtractXrpc(req): ExtractXrpc<GetStatusRequest>,
) -> Result<Json<GetStatusOutput<'static>>, StatusCode> {
    let handle = req.handle;
    let rkey = req.rkey;

    let url = format!(
        "https://public.api.bsky.app/xrpc/com.atproto.identity.resolveHandle?handle={}",
        handle
    );
    let resp = reqwest::get(&url)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !resp.status().is_success() {
        return Err(StatusCode::NOT_FOUND);
    }
    let resp_json: BTreeMap<String, String> = resp
        .json()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let did = resp_json
        .get("did")
        .ok_or(StatusCode::NOT_FOUND)?
        .to_string();

    let at_uri = format!("{}/vg.nat.istat.status.record/{}", did, rkey);

    let row = sqlx::query(
        r#"
        SELECT at, emoji_ref, emoji_ref_cid, title, description, expires, created_at
        FROM statuses
        WHERE at = ?
        "#,
    )
    .bind(&at_uri)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let row = row.ok_or(StatusCode::NOT_FOUND)?;

    let emoji_ref: String = row
        .try_get("emoji_ref")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let title: Option<String> = row.try_get("title").ok();
    let description: Option<String> = row.try_get("description").ok();
    let expires: Option<String> = row.try_get("expires").ok();
    let created_at: String = row
        .try_get("created_at")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let emoji_blob_cid = emoji_ref
        .split('/')
        .last()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let emoji_url = format!(
        "https://cdn.bsky.app/img/avatar/plain/{}/{}@jpeg",
        did, emoji_blob_cid
    );

    let output = GetStatusOutput {
        emoji_url: emoji_url.into(),
        title: title.map(|t| t.into()),
        description: description.map(|d| d.into()),
        expires: expires.map(|e| jacquard_common::types::string::Datetime::raw_str(e)),
        created_at: jacquard_common::types::string::Datetime::raw_str(created_at),
        extra_data: None,
    };

    Ok(Json(output))
}

async fn handle_list_user_statuses(
    State(state): State<AppState>,
    ExtractXrpc(req): ExtractXrpc<ListUserStatusesRequest>,
) -> Result<Json<ListUserStatusesOutput<'static>>, StatusCode> {
    let handle = req.handle;
    let limit = req.limit.unwrap_or(50).min(100) as i64;

    let url = format!(
        "https://public.api.bsky.app/xrpc/com.atproto.identity.resolveHandle?handle={}",
        handle
    );
    let resp = reqwest::get(&url)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !resp.status().is_success() {
        return Err(StatusCode::NOT_FOUND);
    }
    let resp_json: BTreeMap<String, String> = resp
        .json()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let did = resp_json
        .get("did")
        .ok_or(StatusCode::NOT_FOUND)?
        .to_string();

    let rows = sqlx::query(
        r#"
        SELECT rkey, emoji_ref, title, description, expires, created_at
        FROM statuses
        WHERE did = ?
        ORDER BY created_at DESC
        LIMIT ?
        "#,
    )
    .bind(&did)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let statuses: Vec<_> = rows
        .iter()
        .filter_map(|row| {
            let rkey: String = row.try_get("rkey").ok()?;
            let emoji_ref: String = row.try_get("emoji_ref").ok()?;
            let title: Option<String> = row.try_get("title").ok();
            let description: Option<String> = row.try_get("description").ok();
            let expires: Option<String> = row.try_get("expires").ok();
            let created_at: String = row.try_get("created_at").ok()?;

            let emoji_blob_cid = emoji_ref.split('/').last()?;
            let emoji_url = format!(
                "https://cdn.bsky.app/img/avatar/plain/{}/{}@jpeg",
                did, emoji_blob_cid
            );

            let json = serde_json::json!({
                "rkey": rkey,
                "emojiUrl": emoji_url,
                "title": title,
                "description": description,
                "expires": expires,
                "createdAt": created_at
            });

            Some(jacquard_common::types::value::Data::from_json_owned(json).ok()?)
        })
        .collect();

    let output = ListUserStatusesOutput {
        statuses,
        cursor: None,
        extra_data: None,
    };

    Ok(Json(output))
}

async fn handle_list_statuses(
    State(state): State<AppState>,
    ExtractXrpc(req): ExtractXrpc<ListStatusesRequest>,
) -> Result<Json<ListStatusesOutput<'static>>, StatusCode> {
    let limit = req.limit.unwrap_or(50).min(100) as i64;

    let rows = sqlx::query(
        r#"
        SELECT did, rkey, emoji_ref, title, description, expires, created_at
        FROM statuses
        ORDER BY created_at DESC
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let statuses: Vec<_> = rows
        .iter()
        .filter_map(|row| {
            let did: String = row.try_get("did").ok()?;
            let rkey: String = row.try_get("rkey").ok()?;
            let emoji_ref: String = row.try_get("emoji_ref").ok()?;
            let title: Option<String> = row.try_get("title").ok();
            let description: Option<String> = row.try_get("description").ok();
            let expires: Option<String> = row.try_get("expires").ok();
            let created_at: String = row.try_get("created_at").ok()?;

            let emoji_blob_cid = emoji_ref.split('/').last()?;
            let emoji_url = format!(
                "https://cdn.bsky.app/img/avatar/plain/{}/{}@jpeg",
                did, emoji_blob_cid
            );

            // TODO: resolve DIDs to handles - for now just use the DID
            let json = serde_json::json!({
                "did": did,
                "handle": did,
                "rkey": rkey,
                "emojiUrl": emoji_url,
                "title": title,
                "description": description,
                "expires": expires,
                "createdAt": created_at
            });

            Some(jacquard_common::types::value::Data::from_json_owned(json).ok()?)
        })
        .collect();

    let output = ListStatusesOutput {
        statuses,
        cursor: None,
        extra_data: None,
    };

    Ok(Json(output))
}

async fn init_db(db_url: &str) -> Result<SqlitePool> {
    let pool = SqlitePool::connect(db_url).await.into_diagnostic()?;

    let migration_sql = include_str!("../migrations/001_initial_schema.sql");
    sqlx::raw_sql(migration_sql)
        .execute(&pool)
        .await
        .into_diagnostic()?;

    let migration_sql_2 = include_str!("../migrations/002_auth_schema.sql");
    sqlx::raw_sql(migration_sql_2)
        .execute(&pool)
        .await
        .into_diagnostic()?;

    Ok(pool)
}

#[tokio::main]
async fn main() -> Result<()> {
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:istat.db".to_string());
    let pool = init_db(&db_url).await?;

    let jetstream_pool = pool.clone();
    tokio::spawn(async move {
        if let Err(e) = jetstream::start_jetstream(jetstream_pool).await {
            eprintln!("Jetstream error: {}", e);
        }
    });

    let state = AppState {
        db: pool,
        auth_store: oauth::SharedAuthStore::new(),
    };

    let xrpc_router = Router::new()
        .merge(ResolveHandleRequest::into_router(handle_resolve))
        .merge(GetStatusRequest::into_router(handle_get_status))
        .merge(ListUserStatusesRequest::into_router(
            handle_list_user_statuses,
        ))
        .merge(ListStatusesRequest::into_router(handle_list_statuses))
        .fallback(proxy_to_pds)
        .with_state(state.clone());

    let app = Router::new()
        .route("/", axum::routing::get(handle_root))
        .route("/oauth/login", axum::routing::get(oauth::start_login))
        .route(
            "/oauth/callback",
            axum::routing::get(oauth::handle_callback),
        )
        .nest("/xrpc", xrpc_router)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .into_diagnostic()?;
    axum::serve(listener, app).await.unwrap();
    Ok(())
}
