use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::{
        StatusCode,
        header::{HeaderValue, SET_COOKIE},
    },
    response::{Html, IntoResponse, Redirect, Response},
};
use jacquard_common::{session::SessionStoreError, types::did::Did};
use jacquard_oauth::{
    atproto::AtprotoClientMetadata,
    authstore::{ClientAuthStore, MemoryAuthStore},
    client::OAuthClient,
    scopes::Scope,
    session::{AuthRequestData, ClientData, ClientSessionData},
    types::{AuthorizeOptions, CallbackParams},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone)]
pub struct SharedAuthStore {
    inner: Arc<MemoryAuthStore>,
}

impl SharedAuthStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MemoryAuthStore::new()),
        }
    }
}

impl ClientAuthStore for SharedAuthStore {
    fn get_session(
        &self,
        did: &Did<'_>,
        session_id: &str,
    ) -> impl std::future::Future<Output = Result<Option<ClientSessionData<'_>>, SessionStoreError>>
    {
        self.inner.get_session(did, session_id)
    }

    fn upsert_session(
        &self,
        session: ClientSessionData<'_>,
    ) -> impl std::future::Future<Output = Result<(), SessionStoreError>> {
        self.inner.upsert_session(session)
    }

    fn delete_session(
        &self,
        did: &Did<'_>,
        session_id: &str,
    ) -> impl std::future::Future<Output = Result<(), SessionStoreError>> {
        self.inner.delete_session(did, session_id)
    }

    fn get_auth_req_info(
        &self,
        state: &str,
    ) -> impl std::future::Future<Output = Result<Option<AuthRequestData<'_>>, SessionStoreError>>
    {
        self.inner.get_auth_req_info(state)
    }

    fn save_auth_req_info(
        &self,
        auth_req_info: &AuthRequestData<'_>,
    ) -> impl std::future::Future<Output = Result<(), SessionStoreError>> {
        self.inner.save_auth_req_info(auth_req_info)
    }

    fn delete_auth_req_info(
        &self,
        state: &str,
    ) -> impl std::future::Future<Output = Result<(), SessionStoreError>> {
        self.inner.delete_auth_req_info(state)
    }
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: String,
    state: Option<String>,
    iss: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginParams {
    pub handle: String,
}

pub async fn start_login(
    State(state): State<crate::AppState>,
    Query(params): Query<LoginParams>,
) -> Result<Redirect, StatusCode> {
    let redirect_uris = vec!["http://localhost:3000/oauth/callback".parse().unwrap()];

    let config = AtprotoClientMetadata::new_localhost(
        Some(redirect_uris),
        Some(Scope::parse_multiple("atproto transition:generic").unwrap()),
    );

    let client_data = ClientData {
        keyset: None,
        config,
    };

    let oauth = OAuthClient::new(state.auth_store.clone(), client_data);

    match oauth
        .start_auth(params.handle, AuthorizeOptions::default())
        .await
    {
        Ok(auth_url) => Ok(Redirect::to(auth_url.as_str())),
        Err(e) => {
            eprintln!("OAuth start error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn handle_callback(
    State(state): State<crate::AppState>,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, StatusCode> {
    let redirect_uris = vec!["http://localhost:3000/oauth/callback".parse().unwrap()];

    let config = AtprotoClientMetadata::new_localhost(
        Some(redirect_uris),
        Some(Scope::parse_multiple("atproto transition:generic").unwrap()),
    );

    let client_data = ClientData {
        keyset: None,
        config,
    };

    let oauth = OAuthClient::new(state.auth_store.clone(), client_data);

    let params = CallbackParams {
        code: query.code.as_str().into(),
        state: query.state.as_deref().map(|s| s.into()),
        iss: query.iss.as_deref().map(|s| s.into()),
    };

    match oauth.callback(params).await {
        Ok(session) => {
            let session_data = session.data.read().await;
            let did = session_data.account_did.to_string();
            let session_id = session_data.session_id.to_string();
            let access_token = session_data.token_set.access_token.to_string();
            let refresh_token = session_data
                .token_set
                .refresh_token
                .as_ref()
                .map(|t| t.to_string());
            let scope = session_data
                .scopes
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            let now = chrono::Utc::now().to_rfc3339();

            // Use a simplified thumbprint (we can improve this later)
            let dpop_key_thumbprint = format!("dpop_key_{}", session_id);

            // Get expiry time from token set
            let expires_at = session_data
                .token_set
                .expires_at
                .as_ref()
                .map(|dt| dt.as_str().to_string())
                .unwrap_or_else(|| (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339());

            // Save session to database
            sqlx::query(
                r#"
                INSERT OR REPLACE INTO auth_sessions
                (id, did, access_token, refresh_token, dpop_key_thumbprint, scope, created_at, expires_at, last_used_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&session_id)
            .bind(&did)
            .bind(&access_token)
            .bind(refresh_token.as_deref().unwrap_or(""))
            .bind(&dpop_key_thumbprint)
            .bind(&scope)
            .bind(&now)
            .bind(&expires_at)
            .bind(&now)
            .execute(&state.db)
            .await
            .map_err(|e| {
                eprintln!("Failed to save session: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            // Create session cookie
            let cookie = format!(
                "session_id={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000",
                session_id
            );

            eprintln!("Setting cookie: {}", cookie);

            let html = Html(format!(
                "<h1>Login successful!</h1><p>Logged in as: {}</p>",
                did
            ));

            let mut response = html.into_response();
            response
                .headers_mut()
                .insert(SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());

            eprintln!("Response headers: {:?}", response.headers());

            Ok(response)
        }
        Err(e) => {
            eprintln!("OAuth callback error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
