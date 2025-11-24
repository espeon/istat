use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use jacquard_oatproxy::auth::extract_bearer_token;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::{env, str::FromStr};

use crate::AppState;

/// Extract DID from Authorization header by validating JWT
async fn extract_authenticated_did(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<String, StatusCode> {
    let auth_header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Support both "Bearer" and "DPoP" authorization schemes
    let token = extract_bearer_token(auth_header)
        .or_else(|| {
            auth_header
                .strip_prefix("DPoP ")
                .or_else(|| auth_header.strip_prefix("dpop "))
        })
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate the downstream JWT using TokenManager
    let key_store_ref = state.key_store.as_ref();
    let claims = state.token_manager
        .validate_downstream_jwt(token, key_store_ref)
        .await
        .map_err(|e| {
            eprintln!("Failed to validate downstream JWT: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    Ok(claims.sub)
}

/// Check if a DID is an admin
async fn is_admin(did: &str, state: &AppState) -> Result<bool, StatusCode> {
    // First check if this DID matches any initial admin from env var
    // ADMIN_DID can be a single DID or comma-separated list: "did:web:abc,did:web:xyz"
    if let Ok(admin_dids_str) = env::var("ADMIN_DID") {
        let admin_dids: Vec<&str> = admin_dids_str
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if admin_dids.contains(&did) {
            // Ensure this DID is in the admins table
            sqlx::query(
                "INSERT OR IGNORE INTO admins (did, granted_by, notes) VALUES (?, NULL, ?)",
            )
            .bind(did)
            .bind("Initial admin from environment variable")
            .execute(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            return Ok(true);
        }
    }

    // Check database
    let exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM admins WHERE did = ?)")
        .bind(did)
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(exists)
}

/// Require that the authenticated user is an admin
async fn require_admin(headers: &HeaderMap, state: &AppState) -> Result<String, StatusCode> {
    let did = extract_authenticated_did(headers, state).await?;

    if !is_admin(&did, state).await? {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(did)
}

/// Log a moderation action to the audit log
async fn log_audit_action(
    state: &AppState,
    moderator_did: &str,
    action: &str,
    target_type: &str,
    target_id: &str,
    reason: Option<&str>,
    reason_details: Option<&str>,
) -> Result<(), StatusCode> {
    sqlx::query(
        r#"
        INSERT INTO moderation_audit_log
            (moderator_did, action, target_type, target_id, reason, reason_details)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(moderator_did)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(reason)
    .bind(reason_details)
    .execute(&state.db)
    .await
    .map_err(|e| {
        eprintln!("Failed to log audit action: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(())
}

// Request/Response types

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlacklistCidRequest {
    pub cid: String,
    pub reason: String,
    pub reason_details: Option<String>,
    pub content_type: String,
}

#[derive(Debug, Serialize)]
pub struct BlacklistCidResponse {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveBlacklistRequest {
    pub cid: String,
}

#[derive(Debug, Serialize)]
pub struct RemoveBlacklistResponse {
    pub success: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlacklistedCidView {
    pub cid: String,
    pub reason: String,
    pub reason_details: Option<String>,
    pub content_type: String,
    pub moderator_did: String,
    pub blacklisted_at: String,
}

#[derive(Debug, Serialize)]
pub struct ListBlacklistedResponse {
    pub blacklisted: Vec<BlacklistedCidView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IsAdminResponse {
    pub is_admin: bool,
}

#[derive(Debug, Deserialize)]
pub struct DeleteEmojiRequest {
    pub uri: String,
}

#[derive(Debug, Serialize)]
pub struct DeleteEmojiResponse {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct DeleteStatusRequest {
    pub uri: String,
}

#[derive(Debug, Serialize)]
pub struct DeleteStatusResponse {
    pub success: bool,
}

// Endpoint handlers

pub async fn handle_blacklist_cid(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BlacklistCidRequest>,
) -> Result<Json<BlacklistCidResponse>, StatusCode> {
    let moderator_did = require_admin(&headers, &state).await?;

    // Validate reason
    let valid_reasons = ["nudity", "gore", "harassment", "spam", "copyright", "other"];
    if !valid_reasons.contains(&req.reason.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate content_type
    let valid_content_types = ["emoji_blob", "avatar", "banner"];
    if !valid_content_types.contains(&req.content_type.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if already blacklisted
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM blacklisted_cids WHERE cid = ?)",
    )
    .bind(&req.cid)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if exists {
        return Err(StatusCode::CONFLICT);
    }

    // Insert blacklist entry
    sqlx::query(
        r#"
        INSERT INTO blacklisted_cids (cid, reason, reason_details, content_type, moderator_did)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(&req.cid)
    .bind(&req.reason)
    .bind(&req.reason_details)
    .bind(&req.content_type)
    .bind(&moderator_did)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Log audit action
    log_audit_action(
        &state,
        &moderator_did,
        "blacklist_cid",
        &req.content_type,
        &req.cid,
        Some(&req.reason),
        req.reason_details.as_deref(),
    )
    .await?;

    Ok(Json(BlacklistCidResponse { success: true }))
}

pub async fn handle_remove_blacklist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RemoveBlacklistRequest>,
) -> Result<Json<RemoveBlacklistResponse>, StatusCode> {
    let moderator_did = require_admin(&headers, &state).await?;

    // Get the content_type before deleting so we can log it
    let content_type: Option<String> =
        sqlx::query_scalar("SELECT content_type FROM blacklisted_cids WHERE cid = ?")
            .bind(&req.cid)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let content_type = content_type.ok_or(StatusCode::NOT_FOUND)?;

    let result = sqlx::query("DELETE FROM blacklisted_cids WHERE cid = ?")
        .bind(&req.cid)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    // Log audit action
    log_audit_action(
        &state,
        &moderator_did,
        "remove_blacklist",
        &content_type,
        &req.cid,
        None,
        None,
    )
    .await?;

    Ok(Json(RemoveBlacklistResponse { success: true }))
}

pub async fn handle_list_blacklisted(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ListBlacklistedResponse>, StatusCode> {
    let _ = require_admin(&headers, &state).await?;

    let rows = sqlx::query(
        r#"
        SELECT cid, reason, reason_details, content_type, moderator_did, blacklisted_at
        FROM blacklisted_cids
        ORDER BY blacklisted_at DESC
        LIMIT 100
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let blacklisted: Vec<BlacklistedCidView> = rows
        .iter()
        .filter_map(|row| {
            Some(BlacklistedCidView {
                cid: row.try_get("cid").ok()?,
                reason: row.try_get("reason").ok()?,
                reason_details: row.try_get("reason_details").ok().flatten(),
                content_type: row.try_get("content_type").ok()?,
                moderator_did: row.try_get("moderator_did").ok()?,
                blacklisted_at: row.try_get("blacklisted_at").ok()?,
            })
        })
        .collect();

    Ok(Json(ListBlacklistedResponse { blacklisted }))
}

pub async fn handle_is_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<IsAdminResponse>, StatusCode> {
    // Try to extract DID, but if authentication fails, return false instead of 401
    // This allows unauthenticated or invalid token requests to get a meaningful response
    let did = match extract_authenticated_did(&headers, &state).await {
        Ok(did) => did,
        Err(e) => {
            // Not authenticated or invalid token -> not an admin
            eprintln!("Failed to extract DID from auth token (status: {:?})", e);
            return Ok(Json(IsAdminResponse { is_admin: false }));
        }
    };

    let admin = is_admin(&did, &state).await?;

    Ok(Json(IsAdminResponse { is_admin: admin }))
}

pub async fn handle_delete_emoji(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DeleteEmojiRequest>,
) -> Result<Json<DeleteEmojiResponse>, StatusCode> {
    let did = extract_authenticated_did(&headers, &state).await?;
    let is_admin_user = is_admin(&did, &state).await?;

    // Parse AT-URI to get DID and rkey
    // Format: at://did:plc:xyz/vg.nat.istat.moji.emoji/rkey
    let uri_parts: Vec<&str> = req
        .uri
        .strip_prefix("at://")
        .unwrap_or(&req.uri)
        .split('/')
        .collect();
    if uri_parts.len() < 3 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let emoji_did = uri_parts[0];
    let _collection = uri_parts[1];
    let rkey = uri_parts[2];

    // Check if user owns this emoji or is an admin
    if did != emoji_did && !is_admin_user {
        return Err(StatusCode::FORBIDDEN);
    }

    // Soft delete the emoji
    let at_uri_without_prefix = format!("{}/vg.nat.istat.moji.emoji/{}", emoji_did, rkey);
    let result = sqlx::query(
        "UPDATE emojis SET deleted_at = datetime('now'), deleted_by = ? WHERE at = ? AND deleted_at IS NULL"
    )
    .bind(&did)
    .bind(&at_uri_without_prefix)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    // Log audit action
    log_audit_action(&state, &did, "delete_emoji", "emoji", &req.uri, None, None).await?;

    Ok(Json(DeleteEmojiResponse { success: true }))
}

pub async fn handle_delete_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DeleteStatusRequest>,
) -> Result<Json<DeleteStatusResponse>, StatusCode> {
    let did = extract_authenticated_did(&headers, &state).await?;
    let is_admin_user = is_admin(&did, &state).await?;

    // Parse AT-URI to get DID and rkey
    // Format: at://did:plc:xyz/vg.nat.istat.status.record/rkey
    let uri_parts: Vec<&str> = req
        .uri
        .strip_prefix("at://")
        .unwrap_or(&req.uri)
        .split('/')
        .collect();
    if uri_parts.len() < 3 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let status_did = uri_parts[0];
    let _collection = uri_parts[1];
    let rkey = uri_parts[2];

    // Check if user owns this status or is an admin
    if did != status_did && !is_admin_user {
        return Err(StatusCode::FORBIDDEN);
    }

    // Soft delete the status
    let at_uri_without_prefix = format!("{}/vg.nat.istat.status.record/{}", status_did, rkey);
    let result = sqlx::query(
        "UPDATE statuses SET deleted_at = datetime('now'), deleted_by = ? WHERE at = ? AND deleted_at IS NULL"
    )
    .bind(&did)
    .bind(&at_uri_without_prefix)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    // Log audit action
    log_audit_action(
        &state,
        &did,
        "delete_status",
        "status",
        &req.uri,
        None,
        None,
    )
    .await?;

    Ok(Json(DeleteStatusResponse { success: true }))
}

use lexicons::vg_nat::istat::moderation::list_audit_log::{AuditLogEntry, ListAuditLogOutput};

pub async fn handle_list_audit_log(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ListAuditLogOutput<'static>>, StatusCode> {
    let _ = require_admin(&headers, &state).await?;

    let rows = sqlx::query(
        r#"
        SELECT
            l.id,
            l.moderator_did,
            l.action,
            l.target_type,
            l.target_id,
            l.reason,
            l.reason_details,
            l.created_at,
            p.handle as moderator_handle
        FROM moderation_audit_log l
        LEFT JOIN profiles p ON l.moderator_did = p.did
        ORDER BY l.created_at DESC
        LIMIT 100
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    use jacquard_common::types::string::{Datetime, Did, Handle};

    let entries: Vec<_> = rows
        .iter()
        .filter_map(|row| {
            let id: i64 = row.try_get("id").ok()?;
            let moderator_did: String = row.try_get("moderator_did").ok()?;
            let action: String = row.try_get("action").ok()?;
            let target_type: String = row.try_get("target_type").ok()?;
            let target_id: String = row.try_get("target_id").ok()?;
            let reason: Option<String> = row.try_get("reason").ok().flatten();
            let reason_details: Option<String> = row.try_get("reason_details").ok().flatten();
            let created_at: String = row.try_get("created_at").ok()?;
            let moderator_handle: Option<String> = row.try_get("moderator_handle").ok().flatten();

            Some(
                AuditLogEntry::new()
                    .id(id)
                    .moderator_did(Did::from_str(&moderator_did).ok()?)
                    .maybe_moderator_handle(
                        moderator_handle.and_then(|h| Handle::from_str(&h).ok()),
                    )
                    .action(action)
                    .target_type(target_type)
                    .target_id(target_id)
                    .maybe_reason(reason.map(Into::into))
                    .maybe_reason_details(reason_details.map(Into::into))
                    .created_at(Datetime::raw_str(created_at))
                    .build(),
            )
        })
        .collect();

    let output = ListAuditLogOutput {
        entries,
        cursor: None,
        extra_data: None,
    };

    Ok(Json(output))
}
