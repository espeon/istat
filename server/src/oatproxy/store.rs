use async_trait::async_trait;
use jacquard_common::IntoStatic;
use jacquard_oatproxy::{
    error::Result as OatResult,
    session::SessionId,
    store::{DownstreamClientInfo, KeyStore, OAuthSessionStore, PARData, PendingAuth},
};
use p256::ecdsa::SigningKey;
use rand::rngs::OsRng;
use sqlx::{Row, SqlitePool};
use std::sync::Arc;

#[derive(Clone)]
pub struct SqliteStore {
    db: SqlitePool,
    signing_key: SigningKey,
}

impl SqliteStore {
    pub fn builder(db: SqlitePool) -> SqliteStoreBuilder {
        SqliteStoreBuilder {
            db,
            signing_key: None,
        }
    }
}

pub struct SqliteStoreBuilder {
    db: SqlitePool,
    signing_key: Option<SigningKey>,
}

impl SqliteStoreBuilder {
    pub fn with_signing_key(mut self, signing_key: SigningKey) -> Self {
        self.signing_key = Some(signing_key);
        self
    }

    pub fn build(self) -> Arc<SqliteStore> {
        let signing_key = self.signing_key.unwrap_or_else(|| {
            tracing::warn!("No signing key provided, generating temporary key. JWTs will be invalidated on server restart.");
            SigningKey::random(&mut OsRng)
        });

        Arc::new(SqliteStore {
            db: self.db,
            signing_key,
        })
    }
}

#[async_trait]
impl OAuthSessionStore for SqliteStore {
    async fn update_session(
        &self,
        _session: &jacquard_oatproxy::session::OAuthSession,
    ) -> OatResult<()> {
        // Not used - we use ClientAuthStore::upsert_session
        Ok(())
    }

    async fn delete_session(&self, _id: &SessionId) -> OatResult<()> {
        // Not used in current implementation
        Ok(())
    }

    async fn get_by_dpop_jkt(
        &self,
        _jkt: &str,
    ) -> OatResult<Option<jacquard_oatproxy::session::OAuthSession>> {
        // Not used - we look up sessions by DID via ClientAuthStore
        Ok(None)
    }

    async fn store_pending_auth(&self, code: &str, auth: PendingAuth) -> OatResult<()> {
        sqlx::query(
            r#"
            INSERT INTO oatproxy_pending_auths (code, account_did, upstream_session_id, redirect_uri, state, expires_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(code)
        .bind(&auth.account_did)
        .bind(&auth.upstream_session_id)
        .bind(&auth.redirect_uri)
        .bind(&auth.state)
        .bind(auth.expires_at.to_rfc3339())
        .execute(&self.db)
        .await
        .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        Ok(())
    }

    async fn consume_pending_auth(&self, code: &str) -> OatResult<Option<PendingAuth>> {
        let row = sqlx::query(
            r#"
            SELECT account_did, upstream_session_id, redirect_uri, state, expires_at
            FROM oatproxy_pending_auths
            WHERE code = ?
            "#,
        )
        .bind(code)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        if let Some(row) = row {
            // Delete the auth
            sqlx::query("DELETE FROM oatproxy_pending_auths WHERE code = ?")
                .bind(code)
                .execute(&self.db)
                .await
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

            let account_did: String = row
                .try_get("account_did")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;
            let upstream_session_id: String = row
                .try_get("upstream_session_id")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;
            let redirect_uri: String = row
                .try_get("redirect_uri")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;
            let state: Option<String> = row.try_get("state").ok();
            let expires_at: String = row
                .try_get("expires_at")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

            let expires_at = chrono::DateTime::parse_from_rfc3339(&expires_at)
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?
                .with_timezone(&chrono::Utc);

            Ok(Some(PendingAuth {
                account_did,
                upstream_session_id,
                redirect_uri,
                state,
                expires_at,
            }))
        } else {
            Ok(None)
        }
    }

    async fn store_downstream_client_info(
        &self,
        did: &str,
        info: DownstreamClientInfo,
    ) -> OatResult<()> {
        sqlx::query(
            r#"
            INSERT INTO oatproxy_downstream_clients (did, redirect_uri, state, response_type, scope, expires_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(did) DO UPDATE SET
                redirect_uri = excluded.redirect_uri,
                state = excluded.state,
                response_type = excluded.response_type,
                scope = excluded.scope,
                expires_at = excluded.expires_at
            "#,
        )
        .bind(did)
        .bind(&info.redirect_uri)
        .bind(&info.state)
        .bind(&info.response_type)
        .bind(&info.scope)
        .bind(info.expires_at.to_rfc3339())
        .execute(&self.db)
        .await
        .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        Ok(())
    }

    async fn consume_downstream_client_info(
        &self,
        did: &str,
    ) -> OatResult<Option<DownstreamClientInfo>> {
        let row = sqlx::query(
            r#"
            SELECT redirect_uri, state, response_type, scope, expires_at
            FROM oatproxy_downstream_clients
            WHERE did = ?
            "#,
        )
        .bind(did)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        if let Some(row) = row {
            // Delete the client info
            sqlx::query("DELETE FROM oatproxy_downstream_clients WHERE did = ?")
                .bind(did)
                .execute(&self.db)
                .await
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

            let redirect_uri: String = row
                .try_get("redirect_uri")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;
            let state: Option<String> = row.try_get("state").ok();
            let response_type: String = row
                .try_get("response_type")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;
            let scope: Option<String> = row.try_get("scope").ok();
            let expires_at: String = row
                .try_get("expires_at")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

            let expires_at = chrono::DateTime::parse_from_rfc3339(&expires_at)
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?
                .with_timezone(&chrono::Utc);

            Ok(Some(DownstreamClientInfo {
                redirect_uri,
                state,
                response_type,
                scope,
                expires_at,
            }))
        } else {
            Ok(None)
        }
    }

    async fn store_par_data(&self, request_uri: &str, data: PARData) -> OatResult<()> {
        sqlx::query(
            r#"
            INSERT INTO oatproxy_par_data (
                request_uri, client_id, redirect_uri, response_type, state, scope,
                code_challenge, code_challenge_method, login_hint, downstream_dpop_jkt, expires_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(request_uri)
        .bind(&data.client_id)
        .bind(&data.redirect_uri)
        .bind(&data.response_type)
        .bind(&data.state)
        .bind(&data.scope)
        .bind(&data.code_challenge)
        .bind(&data.code_challenge_method)
        .bind(&data.login_hint)
        .bind(&data.downstream_dpop_jkt)
        .bind(data.expires_at.to_rfc3339())
        .execute(&self.db)
        .await
        .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        Ok(())
    }

    async fn consume_par_data(&self, request_uri: &str) -> OatResult<Option<PARData>> {
        let row = sqlx::query(
            r#"
            SELECT client_id, redirect_uri, response_type, state, scope,
                   code_challenge, code_challenge_method, login_hint, downstream_dpop_jkt, expires_at
            FROM oatproxy_par_data
            WHERE request_uri = ?
            "#,
        )
        .bind(request_uri)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        if let Some(row) = row {
            // Delete the PAR data
            sqlx::query("DELETE FROM oatproxy_par_data WHERE request_uri = ?")
                .bind(request_uri)
                .execute(&self.db)
                .await
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

            let client_id: String = row
                .try_get("client_id")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;
            let redirect_uri: String = row
                .try_get("redirect_uri")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;
            let response_type: String = row
                .try_get("response_type")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;
            let state: Option<String> = row.try_get("state").ok();
            let scope: Option<String> = row.try_get("scope").ok();
            let code_challenge: Option<String> = row.try_get("code_challenge").ok();
            let code_challenge_method: Option<String> = row.try_get("code_challenge_method").ok();
            let login_hint: Option<String> = row.try_get("login_hint").ok();
            let downstream_dpop_jkt: String = row
                .try_get("downstream_dpop_jkt")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;
            let expires_at: String = row
                .try_get("expires_at")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

            let expires_at = chrono::DateTime::parse_from_rfc3339(&expires_at)
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?
                .with_timezone(&chrono::Utc);

            Ok(Some(PARData {
                client_id,
                redirect_uri,
                response_type,
                state,
                scope,
                code_challenge,
                code_challenge_method,
                login_hint,
                downstream_dpop_jkt,
                expires_at,
            }))
        } else {
            Ok(None)
        }
    }

    async fn store_refresh_token_mapping(
        &self,
        refresh_token: &str,
        account_did: String,
        session_id: String,
    ) -> OatResult<()> {
        sqlx::query(
            r#"
            INSERT INTO oatproxy_refresh_tokens (refresh_token, account_did, session_id)
            VALUES (?, ?, ?)
            ON CONFLICT(refresh_token) DO UPDATE SET
                account_did = excluded.account_did,
                session_id = excluded.session_id
            "#,
        )
        .bind(refresh_token)
        .bind(&account_did)
        .bind(&session_id)
        .execute(&self.db)
        .await
        .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        Ok(())
    }

    async fn get_refresh_token_mapping(
        &self,
        refresh_token: &str,
    ) -> OatResult<Option<(String, String)>> {
        let row = sqlx::query(
            r#"
            SELECT account_did, session_id
            FROM oatproxy_refresh_tokens
            WHERE refresh_token = ?
            "#,
        )
        .bind(refresh_token)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        if let Some(row) = row {
            let account_did: String = row
                .try_get("account_did")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;
            let session_id: String = row
                .try_get("session_id")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

            Ok(Some((account_did, session_id)))
        } else {
            Ok(None)
        }
    }

    async fn store_active_session(&self, did: &str, session_id: String) -> OatResult<()> {
        sqlx::query(
            r#"
            INSERT INTO oatproxy_active_sessions (did, session_id)
            VALUES (?, ?)
            ON CONFLICT(did) DO UPDATE SET session_id = excluded.session_id
            "#,
        )
        .bind(did)
        .bind(&session_id)
        .execute(&self.db)
        .await
        .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        Ok(())
    }

    async fn get_active_session(&self, did: &str) -> OatResult<Option<String>> {
        let row = sqlx::query(
            r#"
            SELECT session_id
            FROM oatproxy_active_sessions
            WHERE did = ?
            "#,
        )
        .bind(did)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        if let Some(row) = row {
            let session_id: String = row
                .try_get("session_id")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;
            Ok(Some(session_id))
        } else {
            Ok(None)
        }
    }

    async fn store_session_dpop_key(
        &self,
        session_id: &str,
        dpop_jkt: String,
        key: jose_jwk::Jwk,
    ) -> OatResult<()> {
        let key_json = serde_json::to_string(&key)
            .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO oatproxy_session_dpop_keys (session_id, dpop_jkt, key_json)
            VALUES (?, ?, ?)
            ON CONFLICT(session_id) DO UPDATE SET
                dpop_jkt = excluded.dpop_jkt,
                key_json = excluded.key_json
            "#,
        )
        .bind(session_id)
        .bind(&dpop_jkt)
        .bind(&key_json)
        .execute(&self.db)
        .await
        .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        Ok(())
    }

    async fn get_session_dpop_key(
        &self,
        session_id: &str,
    ) -> OatResult<Option<(String, jose_jwk::Jwk)>> {
        let row = sqlx::query(
            r#"
            SELECT dpop_jkt, key_json
            FROM oatproxy_session_dpop_keys
            WHERE session_id = ?
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        if let Some(row) = row {
            let dpop_jkt: String = row
                .try_get("dpop_jkt")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;
            let key_json: String = row
                .try_get("key_json")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

            let key: jose_jwk::Jwk = serde_json::from_str(&key_json)
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

            Ok(Some((dpop_jkt, key)))
        } else {
            Ok(None)
        }
    }

    async fn update_session_dpop_nonce(&self, session_id: &str, nonce: String) -> OatResult<()> {
        sqlx::query(
            r#"
            INSERT INTO oatproxy_session_dpop_nonces (session_id, nonce)
            VALUES (?, ?)
            ON CONFLICT(session_id) DO UPDATE SET nonce = excluded.nonce
            "#,
        )
        .bind(session_id)
        .bind(&nonce)
        .execute(&self.db)
        .await
        .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        Ok(())
    }

    async fn get_session_dpop_nonce(&self, session_id: &str) -> OatResult<Option<String>> {
        let row = sqlx::query(
            r#"
            SELECT nonce
            FROM oatproxy_session_dpop_nonces
            WHERE session_id = ?
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;

        if let Some(row) = row {
            let nonce: String = row
                .try_get("nonce")
                .map_err(|e| jacquard_oatproxy::error::Error::StorageError(e.to_string()))?;
            Ok(Some(nonce))
        } else {
            Ok(None)
        }
    }

    async fn check_and_consume_nonce(&self, jti: &str) -> OatResult<bool> {
        // Try to insert the nonce
        let result = sqlx::query(
            r#"
            INSERT INTO oatproxy_used_nonces (jti, created_at)
            VALUES (?, datetime('now'))
            "#,
        )
        .bind(jti)
        .execute(&self.db)
        .await;

        match result {
            Ok(_) => Ok(true), // Successfully inserted, nonce was valid
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(false), // Already exists, nonce was used
            Err(e) => Err(jacquard_oatproxy::error::Error::StorageError(e.to_string())),
        }
    }
}

#[async_trait]
impl KeyStore for SqliteStore {
    async fn get_signing_key(&self) -> OatResult<SigningKey> {
        Ok(self.signing_key.clone())
    }

    async fn get_dpop_key(&self, _thumbprint: &str) -> OatResult<Option<jose_jwk::Jwk>> {
        // DPoP keys are stored per-session, look them up via get_session_dpop_key instead
        Ok(None)
    }
}

// Implement ClientAuthStore for jacquard-oauth compatibility
#[async_trait]
impl jacquard_oauth::authstore::ClientAuthStore for SqliteStore {
    fn get_session(
        &self,
        account_did: &jacquard_common::types::did::Did<'_>,
        session_id: &str,
    ) -> impl std::future::Future<
        Output = Result<
            Option<jacquard_oauth::session::ClientSessionData<'_>>,
            jacquard_common::session::SessionStoreError,
        >,
    > + Send {
        let did_str = account_did.to_string();
        let session_id = session_id.to_string();
        let db = self.db.clone();

        async move {
            let row = sqlx::query(
                r#"
                SELECT session_data
                FROM oatproxy_oauth_sessions
                WHERE did = ? AND session_id = ?
                "#,
            )
            .bind(&did_str)
            .bind(&session_id)
            .fetch_optional(&db)
            .await
            .map_err(|e| {
                jacquard_common::session::SessionStoreError::Other(e.to_string().into())
            })?;

            if let Some(row) = row {
                let session_data: String = row.try_get("session_data").map_err(|e| {
                    jacquard_common::session::SessionStoreError::Other(e.to_string().into())
                })?;

                let session: jacquard_oauth::session::ClientSessionData<'_> =
                    serde_json::from_str(&session_data)
                        .map_err(|e| jacquard_common::session::SessionStoreError::Serde(e))?;

                Ok(Some(session.into_static()))
            } else {
                Ok(None)
            }
        }
    }

    fn upsert_session(
        &self,
        session_data: jacquard_oauth::session::ClientSessionData<'_>,
    ) -> impl std::future::Future<Output = Result<(), jacquard_common::session::SessionStoreError>> + Send
    {
        let db = self.db.clone();

        async move {
            let did_str = session_data.account_did.to_string();
            let session_id = session_data.session_id.to_string();
            let serialized = serde_json::to_string(&session_data)
                .map_err(|e| jacquard_common::session::SessionStoreError::Serde(e))?;

            sqlx::query(
                r#"
                INSERT INTO oatproxy_oauth_sessions (did, session_id, session_data)
                VALUES (?, ?, ?)
                ON CONFLICT(did, session_id) DO UPDATE SET session_data = excluded.session_data
                "#,
            )
            .bind(&did_str)
            .bind(&session_id)
            .bind(&serialized)
            .execute(&db)
            .await
            .map_err(|e| {
                jacquard_common::session::SessionStoreError::Other(e.to_string().into())
            })?;

            Ok(())
        }
    }

    fn delete_session(
        &self,
        account_did: &jacquard_common::types::did::Did<'_>,
        session_id: &str,
    ) -> impl std::future::Future<Output = Result<(), jacquard_common::session::SessionStoreError>> + Send
    {
        let did_str = account_did.to_string();
        let session_id = session_id.to_string();
        let db = self.db.clone();

        async move {
            sqlx::query(
                r#"
                DELETE FROM oatproxy_oauth_sessions
                WHERE did = ? AND session_id = ?
                "#,
            )
            .bind(&did_str)
            .bind(&session_id)
            .execute(&db)
            .await
            .map_err(|e| {
                jacquard_common::session::SessionStoreError::Other(e.to_string().into())
            })?;

            Ok(())
        }
    }

    fn get_auth_req_info(
        &self,
        state: &str,
    ) -> impl std::future::Future<
        Output = Result<
            Option<jacquard_oauth::session::AuthRequestData<'_>>,
            jacquard_common::session::SessionStoreError,
        >,
    > + Send {
        let state = state.to_string();
        let db = self.db.clone();

        async move {
            let row = sqlx::query(
                r#"
                SELECT auth_req_data
                FROM oatproxy_auth_requests
                WHERE state = ?
                "#,
            )
            .bind(&state)
            .fetch_optional(&db)
            .await
            .map_err(|e| {
                jacquard_common::session::SessionStoreError::Other(e.to_string().into())
            })?;

            if let Some(row) = row {
                let auth_req_data: String = row.try_get("auth_req_data").map_err(|e| {
                    jacquard_common::session::SessionStoreError::Other(e.to_string().into())
                })?;

                let auth_req: jacquard_oauth::session::AuthRequestData<'_> =
                    serde_json::from_str(&auth_req_data)
                        .map_err(|e| jacquard_common::session::SessionStoreError::Serde(e))?;

                Ok(Some(auth_req.into_static()))
            } else {
                Ok(None)
            }
        }
    }

    fn save_auth_req_info(
        &self,
        auth_req_info: &jacquard_oauth::session::AuthRequestData<'_>,
    ) -> impl std::future::Future<Output = Result<(), jacquard_common::session::SessionStoreError>> + Send
    {
        let state = auth_req_info.state.to_string();
        let serialized = serde_json::to_string(auth_req_info)
            .map_err(|e| jacquard_common::session::SessionStoreError::Serde(e));
        let db = self.db.clone();

        async move {
            let data = serialized?;

            sqlx::query(
                r#"
                INSERT INTO oatproxy_auth_requests (state, auth_req_data)
                VALUES (?, ?)
                ON CONFLICT(state) DO UPDATE SET auth_req_data = excluded.auth_req_data
                "#,
            )
            .bind(&state)
            .bind(&data)
            .execute(&db)
            .await
            .map_err(|e| {
                jacquard_common::session::SessionStoreError::Other(e.to_string().into())
            })?;

            Ok(())
        }
    }

    fn delete_auth_req_info(
        &self,
        state: &str,
    ) -> impl std::future::Future<Output = Result<(), jacquard_common::session::SessionStoreError>> + Send
    {
        let state = state.to_string();
        let db = self.db.clone();

        async move {
            sqlx::query(
                r#"
                DELETE FROM oatproxy_auth_requests
                WHERE state = ?
                "#,
            )
            .bind(&state)
            .execute(&db)
            .await
            .map_err(|e| {
                jacquard_common::session::SessionStoreError::Other(e.to_string().into())
            })?;

            Ok(())
        }
    }
}
