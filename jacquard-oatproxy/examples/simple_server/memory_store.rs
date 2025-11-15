use async_trait::async_trait;
use chrono::{DateTime, Utc};
use jacquard_common::IntoStatic;
use jacquard_oatproxy::{
    error::Result,
    session::{OAuthSession, SessionId},
    store::{DownstreamClientInfo, KeyStore, NonceStore, OAuthSessionStore, PARData, PendingAuth},
};
use p256::ecdsa::SigningKey;
use rand::rngs::OsRng;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct MemoryStore {
    sessions: Arc<RwLock<HashMap<SessionId, OAuthSession>>>,
    pending_auths: Arc<RwLock<HashMap<String, PendingAuth>>>,
    downstream_clients: Arc<RwLock<HashMap<String, DownstreamClientInfo>>>,
    par_data: Arc<RwLock<HashMap<String, PARData>>>,
    refresh_tokens: Arc<RwLock<HashMap<String, (String, String)>>>, // refresh_token -> (did, session_id)
    active_sessions: Arc<RwLock<HashMap<String, String>>>,          // did -> session_id
    session_dpop_keys: Arc<RwLock<HashMap<String, (String, jose_jwk::Jwk)>>>, // session_id -> (jkt, key)
    session_dpop_nonces: Arc<RwLock<HashMap<String, String>>>,                // session_id -> nonce
    nonce_pads: Arc<RwLock<HashMap<String, String>>>, // session_id -> nonce_pad
    signing_key: SigningKey,
    used_nonces: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
    // jacquard-oauth storage
    auth_requests: Arc<RwLock<HashMap<String, String>>>, // state -> JSON serialized AuthRequestData
    oauth_sessions: Arc<RwLock<HashMap<(String, String), String>>>, // (did, session_id) -> JSON serialized ClientSessionData
}

impl MemoryStore {
    pub fn new() -> Self {
        // Generate a signing key for the proxy
        let signing_key = SigningKey::random(&mut OsRng);

        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            pending_auths: Arc::new(RwLock::new(HashMap::new())),
            downstream_clients: Arc::new(RwLock::new(HashMap::new())),
            par_data: Arc::new(RwLock::new(HashMap::new())),
            refresh_tokens: Arc::new(RwLock::new(HashMap::new())),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            session_dpop_keys: Arc::new(RwLock::new(HashMap::new())),
            session_dpop_nonces: Arc::new(RwLock::new(HashMap::new())),
            nonce_pads: Arc::new(RwLock::new(HashMap::new())),
            signing_key,
            used_nonces: Arc::new(RwLock::new(HashMap::new())),
            auth_requests: Arc::new(RwLock::new(HashMap::new())),
            oauth_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl OAuthSessionStore for MemoryStore {
    async fn create_session(&self, session: OAuthSession) -> Result<SessionId> {
        let id = session.id.clone();
        self.sessions.write().unwrap().insert(id.clone(), session);
        Ok(id)
    }

    async fn get_session(&self, id: &SessionId) -> Result<Option<OAuthSession>> {
        Ok(self.sessions.read().unwrap().get(id).cloned())
    }

    async fn update_session(&self, session: &OAuthSession) -> Result<()> {
        self.sessions
            .write()
            .unwrap()
            .insert(session.id.clone(), session.clone());
        Ok(())
    }

    async fn delete_session(&self, id: &SessionId) -> Result<()> {
        self.sessions.write().unwrap().remove(id);
        Ok(())
    }

    async fn get_by_request_uri(&self, _uri: &str) -> Result<Option<OAuthSession>> {
        // Not used in this implementation
        Ok(None)
    }

    async fn get_by_state(&self, state: &str) -> Result<Option<OAuthSession>> {
        Ok(self
            .sessions
            .read()
            .unwrap()
            .values()
            .find(|s| s.downstream_state.as_deref() == Some(state))
            .cloned())
    }

    async fn get_by_dpop_jkt(&self, jkt: &str) -> Result<Option<OAuthSession>> {
        Ok(self
            .sessions
            .read()
            .unwrap()
            .values()
            .find(|s| s.downstream_dpop_key_thumbprint == jkt)
            .cloned())
    }

    async fn store_pending_auth(&self, code: &str, auth: PendingAuth) -> Result<()> {
        self.pending_auths
            .write()
            .unwrap()
            .insert(code.to_string(), auth);
        Ok(())
    }

    async fn consume_pending_auth(&self, code: &str) -> Result<Option<PendingAuth>> {
        Ok(self.pending_auths.write().unwrap().remove(code))
    }

    async fn store_downstream_client_info(
        &self,
        did: &str,
        info: DownstreamClientInfo,
    ) -> Result<()> {
        self.downstream_clients
            .write()
            .unwrap()
            .insert(did.to_string(), info);
        Ok(())
    }

    async fn consume_downstream_client_info(
        &self,
        did: &str,
    ) -> Result<Option<DownstreamClientInfo>> {
        Ok(self.downstream_clients.write().unwrap().remove(did))
    }

    async fn store_par_data(&self, request_uri: &str, data: PARData) -> Result<()> {
        self.par_data
            .write()
            .unwrap()
            .insert(request_uri.to_string(), data);
        Ok(())
    }

    async fn consume_par_data(&self, request_uri: &str) -> Result<Option<PARData>> {
        Ok(self.par_data.write().unwrap().remove(request_uri))
    }

    async fn store_refresh_token_mapping(
        &self,
        refresh_token: &str,
        account_did: String,
        session_id: String,
    ) -> Result<()> {
        self.refresh_tokens
            .write()
            .unwrap()
            .insert(refresh_token.to_string(), (account_did, session_id));
        Ok(())
    }

    async fn get_refresh_token_mapping(
        &self,
        refresh_token: &str,
    ) -> Result<Option<(String, String)>> {
        Ok(self
            .refresh_tokens
            .read()
            .unwrap()
            .get(refresh_token)
            .cloned())
    }

    async fn store_active_session(&self, did: &str, session_id: String) -> Result<()> {
        self.active_sessions
            .write()
            .unwrap()
            .insert(did.to_string(), session_id);
        Ok(())
    }

    async fn get_active_session(&self, did: &str) -> Result<Option<String>> {
        Ok(self.active_sessions.read().unwrap().get(did).cloned())
    }

    async fn store_session_dpop_key(
        &self,
        session_id: &str,
        dpop_jkt: String,
        key: jose_jwk::Jwk,
    ) -> Result<()> {
        self.session_dpop_keys
            .write()
            .unwrap()
            .insert(session_id.to_string(), (dpop_jkt, key));
        Ok(())
    }

    async fn get_session_dpop_key(
        &self,
        session_id: &str,
    ) -> Result<Option<(String, jose_jwk::Jwk)>> {
        Ok(self
            .session_dpop_keys
            .read()
            .unwrap()
            .get(session_id)
            .cloned())
    }

    async fn update_session_dpop_nonce(&self, session_id: &str, nonce: String) -> Result<()> {
        self.session_dpop_nonces
            .write()
            .unwrap()
            .insert(session_id.to_string(), nonce);
        Ok(())
    }

    async fn get_session_dpop_nonce(&self, session_id: &str) -> Result<Option<String>> {
        Ok(self
            .session_dpop_nonces
            .read()
            .unwrap()
            .get(session_id)
            .cloned())
    }
}

#[async_trait]
impl KeyStore for MemoryStore {
    async fn get_signing_key(&self) -> Result<SigningKey> {
        Ok(self.signing_key.clone())
    }

    async fn create_dpop_key(&self) -> Result<jose_jwk::Jwk> {
        // Generate a new P256 key for DPoP
        let signing_key = SigningKey::random(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        // Convert to JWK
        let jwk = jose_jwk::Jwk {
            key: jose_jwk::Key::Ec(jose_jwk::Ec {
                crv: jose_jwk::EcCurves::P256,
                x: verifying_key
                    .to_encoded_point(false)
                    .x()
                    .unwrap()
                    .to_vec()
                    .into(),
                y: verifying_key
                    .to_encoded_point(false)
                    .y()
                    .unwrap()
                    .to_vec()
                    .into(),
                d: Some(signing_key.to_bytes().to_vec().into()),
            }),
            prm: jose_jwk::Parameters::default(),
        };

        Ok(jwk)
    }

    async fn get_dpop_key(&self, thumbprint: &str) -> Result<Option<jose_jwk::Jwk>> {
        // Search through stored session keys
        Ok(self
            .session_dpop_keys
            .read()
            .unwrap()
            .values()
            .find(|(jkt, _)| jkt == thumbprint)
            .map(|(_, key)| key.clone()))
    }
}

#[async_trait]
impl NonceStore for MemoryStore {
    async fn check_and_consume_nonce(&self, jti: &str) -> Result<bool> {
        let mut nonces = self.used_nonces.write().unwrap();

        // Check if already used
        if nonces.contains_key(jti) {
            return Ok(false);
        }

        // Mark as used
        nonces.insert(jti.to_string(), Utc::now());
        Ok(true)
    }

    async fn generate_nonce(&self, session_id: &str, nonce_pad: &str) -> Result<String> {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Generate random bytes for the nonce
        let nonce_bytes: [u8; 16] = rng.r#gen();

        // XOR with nonce_pad bytes
        let pad_bytes = hex::decode(nonce_pad).unwrap_or_else(|_| nonce_pad.as_bytes().to_vec());
        let mut result = nonce_bytes.to_vec();

        for (i, byte) in result.iter_mut().enumerate() {
            if i < pad_bytes.len() {
                *byte ^= pad_bytes[i];
            }
        }

        // Store the nonce for this session
        let nonce_hex = hex::encode(&result);
        self.session_dpop_nonces
            .write()
            .unwrap()
            .insert(session_id.to_string(), nonce_hex.clone());

        Ok(nonce_hex)
    }

    async fn store_nonce_pad(&self, session_id: &str, nonce_pad: &str) -> Result<()> {
        self.nonce_pads
            .write()
            .unwrap()
            .insert(session_id.to_string(), nonce_pad.to_string());
        Ok(())
    }

    async fn get_nonce_pad(&self, session_id: &str) -> Result<Option<String>> {
        Ok(self.nonce_pads.read().unwrap().get(session_id).cloned())
    }

    async fn verify_nonce(&self, session_id: &str, nonce: &str) -> Result<bool> {
        // Get the nonce pad for this session
        let nonce_pad = match self.nonce_pads.read().unwrap().get(session_id) {
            Some(pad) => pad.clone(),
            None => return Ok(false),
        };

        // Get the last nonce we sent
        let last_nonce = match self.session_dpop_nonces.read().unwrap().get(session_id) {
            Some(n) => n.clone(),
            None => return Ok(false),
        };

        // The client should have XOR'd the nonce we sent with the nonce_pad
        // So we XOR it again to get back the original random value
        let nonce_bytes = match hex::decode(nonce) {
            Ok(bytes) => bytes,
            Err(_) => return Ok(false),
        };

        let pad_bytes = hex::decode(&nonce_pad).unwrap_or_else(|_| nonce_pad.as_bytes().to_vec());

        let mut unxored = nonce_bytes.clone();
        for (i, byte) in unxored.iter_mut().enumerate() {
            if i < pad_bytes.len() {
                *byte ^= pad_bytes[i];
            }
        }

        // XOR the last nonce we sent to get its original value
        let last_nonce_bytes = match hex::decode(&last_nonce) {
            Ok(bytes) => bytes,
            Err(_) => return Ok(false),
        };

        let mut last_unxored = last_nonce_bytes.clone();
        for (i, byte) in last_unxored.iter_mut().enumerate() {
            if i < pad_bytes.len() {
                *byte ^= pad_bytes[i];
            }
        }

        // They should not match (client must have used a fresh nonce)
        Ok(unxored != last_unxored)
    }

    async fn cleanup_expired(&self, before: DateTime<Utc>) -> Result<()> {
        self.used_nonces
            .write()
            .unwrap()
            .retain(|_, timestamp| *timestamp > before);
        Ok(())
    }
}

// Implement ClientAuthStore trait for jacquard-oauth compatibility
#[async_trait]
impl jacquard_oauth::authstore::ClientAuthStore for MemoryStore {
    fn get_session(
        &self,
        account_did: &jacquard_common::types::did::Did<'_>,
        session_id: &str,
    ) -> impl std::future::Future<
        Output = std::result::Result<
            Option<jacquard_oauth::session::ClientSessionData<'_>>,
            jacquard_common::session::SessionStoreError,
        >,
    > + Send {
        let did_str = account_did.to_string();
        let session_id = session_id.to_string();
        let oauth_sessions = self.oauth_sessions.clone();

        async move {
            let sessions = oauth_sessions.read().unwrap();
            if let Some(data) = sessions.get(&(did_str, session_id)) {
                // Clone the data to avoid lifetime issues
                let data_owned = data.clone().to_owned();
                drop(sessions); // release lock

                let session_data: jacquard_oauth::session::ClientSessionData<'_> =
                    serde_json::from_str(&data_owned)
                        .map_err(|e| jacquard_common::session::SessionStoreError::Serde(e))?;

                Ok(Some(session_data.into_static()))
            } else {
                Ok(None)
            }
        }
    }

    fn upsert_session(
        &self,
        session_data: jacquard_oauth::session::ClientSessionData<'_>,
    ) -> impl std::future::Future<
        Output = std::result::Result<(), jacquard_common::session::SessionStoreError>,
    > + Send {
        let oauth_sessions = self.oauth_sessions.clone();

        async move {
            let did_str = session_data.account_did.to_string();
            let session_id = session_data.session_id.to_string();
            let serialized = serde_json::to_string(&session_data)
                .map_err(|e| jacquard_common::session::SessionStoreError::Serde(e))?;

            oauth_sessions
                .write()
                .unwrap()
                .insert((did_str, session_id), serialized);
            Ok(())
        }
    }

    fn delete_session(
        &self,
        account_did: &jacquard_common::types::did::Did<'_>,
        session_id: &str,
    ) -> impl std::future::Future<
        Output = std::result::Result<(), jacquard_common::session::SessionStoreError>,
    > + Send {
        let did_str = account_did.to_string();
        let session_id = session_id.to_string();
        let oauth_sessions = self.oauth_sessions.clone();

        async move {
            oauth_sessions
                .write()
                .unwrap()
                .remove(&(did_str, session_id));
            Ok(())
        }
    }

    fn get_auth_req_info(
        &self,
        state: &str,
    ) -> impl std::future::Future<
        Output = std::result::Result<
            Option<jacquard_oauth::session::AuthRequestData<'_>>,
            jacquard_common::session::SessionStoreError,
        >,
    > + Send {
        let state = state.to_string();
        let auth_requests = self.auth_requests.clone();

        async move {
            let requests = auth_requests.read().unwrap();
            if let Some(data) = requests.get(&state) {
                // Clone the data to avoid lifetime issues
                let data_owned = data.clone();
                drop(requests); // release lock

                let auth_req: jacquard_oauth::session::AuthRequestData<'_> =
                    serde_json::from_str(&data_owned)
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
    ) -> impl std::future::Future<
        Output = std::result::Result<(), jacquard_common::session::SessionStoreError>,
    > + Send {
        let state = auth_req_info.state.to_string();
        let auth_requests = self.auth_requests.clone();
        let serialized = serde_json::to_string(auth_req_info)
            .map_err(|e| jacquard_common::session::SessionStoreError::Serde(e));

        async move {
            let data = serialized?;
            auth_requests.write().unwrap().insert(state, data);
            Ok(())
        }
    }

    fn delete_auth_req_info(
        &self,
        state: &str,
    ) -> impl std::future::Future<
        Output = std::result::Result<(), jacquard_common::session::SessionStoreError>,
    > + Send {
        let state = state.to_string();
        let auth_requests = self.auth_requests.clone();

        async move {
            auth_requests.write().unwrap().remove(&state);
            Ok(())
        }
    }
}
