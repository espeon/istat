use axum::{Json, extract::State, http::StatusCode};
use jacquard::api::com_atproto::identity::resolve_handle::{
    ResolveHandleOutput, ResolveHandleRequest,
};
use jacquard_axum::ExtractXrpc;
use jacquard_common::types::string::Did;
use lexicons::vg_nat::istat::{
    actor::get_profile::{GetProfileOutput, GetProfileRequest},
    moji::search_emoji::{SearchEmojiOutput, SearchEmojiRequest},
    status::{
        get_status::{GetStatusOutput, GetStatusRequest},
        list_statuses::{ListStatusesOutput, ListStatusesRequest},
        list_user_statuses::{ListUserStatusesOutput, ListUserStatusesRequest},
    },
};
use sqlx::Row;
use std::{collections::BTreeMap, str::FromStr};

use crate::AppState;

pub async fn handle_resolve(
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

pub async fn handle_get_status(
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
        SELECT s.at, s.emoji_ref, s.emoji_ref_cid, s.title, s.description, s.expires, s.created_at,
               e.mime_type
        FROM statuses s
        LEFT JOIN emojis e ON s.emoji_ref = 'at://' || e.at
        WHERE s.at = ?
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
    let mime_type: Option<String> = row.try_get("mime_type").ok().flatten();
    let title: Option<String> = row.try_get("title").ok();
    let description: Option<String> = row.try_get("description").ok();
    let expires: Option<String> = row.try_get("expires").ok();
    let created_at: String = row
        .try_get("created_at")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mime_ext = mime_type
        .as_deref()
        .and_then(|m| match m {
            "image/png" => Some("png"),
            "image/jpeg" => Some("jpeg"),
            "image/jpg" => Some("jpeg"),
            "image/webp" => Some("webp"),
            "image/gif" => Some("gif"),
            _ => Some("jpeg"),
        })
        .unwrap_or("jpeg");

    let emoji_blob_cid = emoji_ref
        .split('/')
        .last()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let emoji_url = format!(
        "https://at.uwu.wang/{}/{}@{}",
        did, emoji_blob_cid, mime_ext
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

pub async fn handle_get_profile(
    State(state): State<AppState>,
    ExtractXrpc(req): ExtractXrpc<GetProfileRequest>,
) -> Result<Json<GetProfileOutput<'static>>, StatusCode> {
    let actor = req.actor;

    // resolve to DID if it's a handle
    let did = if actor.as_str().starts_with("did:") {
        actor.to_string()
    } else {
        let url = format!(
            "https://public.api.bsky.app/xrpc/com.atproto.identity.resolveHandle?handle={}",
            actor
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
        resp_json
            .get("did")
            .ok_or(StatusCode::NOT_FOUND)?
            .to_string()
    };

    let row = sqlx::query(
        r#"
        SELECT did, handle, display_name, description, avatar_cid, banner_cid,
               pronouns, website, created_at
        FROM profiles
        WHERE did = ?
        "#,
    )
    .bind(&did)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let row = row.ok_or(StatusCode::NOT_FOUND)?;

    use jacquard_common::types::string::{Datetime, Did as DidType, Handle};

    let handle: String = row
        .try_get("handle")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let display_name: Option<String> = row.try_get("display_name").ok().flatten();
    let description: Option<String> = row.try_get("description").ok().flatten();
    let avatar_cid: Option<String> = row.try_get("avatar_cid").ok().flatten();
    let banner_cid: Option<String> = row.try_get("banner_cid").ok().flatten();
    let pronouns: Option<String> = row.try_get("pronouns").ok().flatten();
    let website: Option<String> = row.try_get("website").ok().flatten();
    let created_at: Option<String> = row.try_get("created_at").ok().flatten();

    let avatar = avatar_cid.map(|cid| format!("https://at.uwu.wang/{}/{}@webp", did, cid));
    let banner = banner_cid.map(|cid| format!("https://at.uwu.wang/{}/{}@webp", did, cid));

    let output = GetProfileOutput {
        did: DidType::from_str(&did).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        handle: Handle::from_str(&handle).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        display_name: display_name.map(Into::into),
        description: description.map(Into::into),
        avatar: avatar.map(Into::into),
        banner: banner.map(Into::into),
        pronouns: pronouns.map(Into::into),
        website: website.map(Into::into),
        created_at: created_at
            .filter(|s| !s.is_empty() && s.contains('T'))
            .map(|s| Datetime::raw_str(s)),
        extra_data: None,
    };

    Ok(Json(output))
}

pub async fn handle_search_emoji(
    State(state): State<AppState>,
    ExtractXrpc(req): ExtractXrpc<SearchEmojiRequest>,
) -> Result<Json<SearchEmojiOutput<'static>>, StatusCode> {
    let query = req.query;
    let limit = req.limit.unwrap_or(20).min(100) as i64;

    // Use LIKE for simple case-insensitive search
    // SQLite FTS would be better for production, but this works for now
    let search_pattern = format!("%{}%", query);

    let rows = sqlx::query(
        r#"
        SELECT e.at, e.did, e.blob_cid, e.mime_type, e.emoji_name, e.alt_text,
               p.handle
        FROM emojis e
        LEFT JOIN profiles p ON e.did = p.did
        WHERE e.emoji_name LIKE ? COLLATE NOCASE
           OR e.alt_text LIKE ? COLLATE NOCASE
        ORDER BY e.created_at DESC
        LIMIT ?
        "#,
    )
    .bind(&search_pattern)
    .bind(&search_pattern)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    eprintln!("search_emoji query='{}' found {} rows", query, rows.len());

    use jacquard_common::types::string::{AtUri, Did as DidType, Handle};
    use lexicons::vg_nat::istat::moji::search_emoji::EmojiView;

    let emojis: Vec<_> = rows
        .iter()
        .filter_map(|row| {
            let at_uri_without_prefix: String = row.try_get("at").ok()?;
            let at_uri = format!("at://{}", at_uri_without_prefix);
            let did: String = row.try_get("did").ok()?;
            let blob_cid: String = row.try_get("blob_cid").ok()?;
            let mime_type: Option<String> = row.try_get("mime_type").ok().flatten();
            let emoji_name: Option<String> = row.try_get("emoji_name").ok().flatten();
            let alt_text: Option<String> = row.try_get("alt_text").ok().flatten();
            let handle: Option<String> = row.try_get("handle").ok().flatten();

            eprintln!(
                "processing emoji: uri={}, name={:?}, alt={:?}",
                at_uri, emoji_name, alt_text
            );

            let mime_ext = mime_type
                .as_deref()
                .and_then(|m| match m {
                    "image/png" => Some("png"),
                    "image/jpeg" => Some("jpeg"),
                    "image/jpg" => Some("jpeg"),
                    "image/webp" => Some("webp"),
                    "image/gif" => Some("gif"),
                    _ => Some("jpeg"),
                })
                .unwrap_or("jpeg");

            let url = format!("https://at.uwu.wang/{}/{}@{}", did, blob_cid, mime_ext);

            let result = EmojiView::new()
                .uri(AtUri::from_str(&at_uri).ok()?)
                .name(emoji_name.unwrap_or_else(|| "changeme".to_string()))
                .maybe_alt_text(alt_text.map(Into::into))
                .url(url)
                .created_by(DidType::from_str(&did).ok()?)
                .maybe_created_by_handle(handle.and_then(|h| Handle::from_str(&h).ok()))
                .build();

            eprintln!("successfully built emoji view");
            Some(result)
        })
        .collect();

    let output = SearchEmojiOutput {
        emojis,
        extra_data: None,
    };

    Ok(Json(output))
}

pub async fn handle_list_user_statuses(
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
        SELECT s.rkey, s.emoji_ref, s.title, s.description, s.expires, s.created_at,
               p.handle, p.display_name, p.avatar_cid,
               e.blob_cid as emoji_blob_cid, e.mime_type, e.emoji_name, e.alt_text, e.did as emoji_did
        FROM statuses s
        LEFT JOIN profiles p ON s.did = p.did
        LEFT JOIN emojis e ON s.emoji_ref = 'at://' || e.at
        WHERE s.did = ?
          AND (s.expires IS NULL OR datetime(s.expires) > datetime('now'))
        ORDER BY s.created_at DESC
        LIMIT ?
        "#,
    )
    .bind(&did)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    use jacquard_common::types::string::Datetime;
    use lexicons::vg_nat::istat::status::list_user_statuses::UserStatusView;

    let statuses: Vec<_> = rows
        .iter()
        .filter_map(|row| {
            let rkey: String = row.try_get("rkey").ok()?;
            let emoji_ref: String = row.try_get("emoji_ref").ok()?;
            let emoji_blob_cid: Option<String> = row.try_get("emoji_blob_cid").ok().flatten();
            let mime_type: Option<String> = row.try_get("mime_type").ok().flatten();
            let title: Option<String> = row
                .try_get("title")
                .ok()
                .and_then(|s: String| if s.is_empty() { None } else { Some(s) });
            let description: Option<String> = row
                .try_get("description")
                .ok()
                .and_then(|s: String| if s.is_empty() { None } else { Some(s) });
            let expires: Option<String> = row.try_get("expires").ok();
            let created_at: String = row.try_get("created_at").ok()?;
            let handle: Option<String> = row.try_get("handle").ok().flatten();
            let display_name: Option<String> = row.try_get("display_name").ok().flatten();
            let avatar_cid: Option<String> = row.try_get("avatar_cid").ok().flatten();
            let emoji_name: Option<String> = row.try_get("emoji_name").ok().flatten();
            let alt_text: Option<String> = row.try_get("alt_text").ok().flatten();
            let emoji_did: Option<String> = row.try_get("emoji_did").ok().flatten();

            let mime_ext = mime_type
                .as_deref()
                .and_then(|m| match m {
                    "image/png" => Some("png"),
                    "image/jpeg" => Some("jpeg"),
                    "image/jpg" => Some("jpeg"),
                    "image/webp" => Some("webp"),
                    "image/gif" => Some("gif"),
                    _ => Some("jpeg"),
                })
                .unwrap_or("jpeg");

            let emoji_url = if let Some(blob_cid) = emoji_blob_cid {
                if let Some(emoji_owner_did) = emoji_did {
                    format!(
                        "https://at.uwu.wang/{}/{}@{}",
                        emoji_owner_did, blob_cid, mime_ext
                    )
                } else {
                    // fallback: try to extract DID from emoji_ref
                    emoji_ref
                        .strip_prefix("at://")
                        .and_then(|s| s.split('/').next())
                        .map(|emoji_owner| {
                            format!(
                                "https://at.uwu.wang/{}/{}@{}",
                                emoji_owner, blob_cid, mime_ext
                            )
                        })
                        .unwrap_or_else(|| {
                            format!("https://at.uwu.wang/{}/{}@{}", did, blob_cid, mime_ext)
                        })
                }
            } else {
                emoji_ref
                    .split('/')
                    .last()
                    .map(|cid| format!("https://at.uwu.wang/{}/{}@{}", did, cid, mime_ext))
                    .unwrap_or_else(|| {
                        eprintln!(
                            "Warning: emoji not found for user status {}, emoji_ref: {}",
                            rkey, emoji_ref
                        );
                        String::new()
                    })
            };

            let avatar_url =
                avatar_cid.map(|cid| format!("https://at.uwu.wang/{}/{}@webp", did, cid));

            // Validate datetime format before passing to raw_str to avoid panics
            // Skip statuses with invalid datetimes
            if created_at.is_empty() || !created_at.contains('T') {
                eprintln!("Invalid created_at datetime for status: {}", created_at);
                return None;
            }

            Some(
                UserStatusView::new()
                    .maybe_handle(handle.map(Into::into))
                    .maybe_display_name(display_name.map(Into::into))
                    .maybe_avatar_url(avatar_url.map(Into::into))
                    .rkey(rkey)
                    .emoji_url(emoji_url)
                    .maybe_emoji_name(emoji_name.map(Into::into))
                    .maybe_emoji_alt(alt_text.map(Into::into))
                    .maybe_title(title.map(Into::into))
                    .maybe_description(description.map(Into::into))
                    .maybe_expires(
                        expires
                            .filter(|e| !e.is_empty() && e.contains('T'))
                            .map(|e| Datetime::raw_str(e)),
                    )
                    .created_at(Datetime::raw_str(created_at))
                    .build(),
            )
        })
        .collect();

    let output = ListUserStatusesOutput {
        statuses,
        cursor: None,
        extra_data: None,
    };

    Ok(Json(output))
}

pub async fn handle_list_statuses(
    State(state): State<AppState>,
    ExtractXrpc(req): ExtractXrpc<ListStatusesRequest>,
) -> Result<Json<ListStatusesOutput<'static>>, StatusCode> {
    let limit = req.limit.unwrap_or(50).min(100) as i64;

    let rows = sqlx::query(
        r#"
        SELECT s.did, s.rkey, s.emoji_ref, s.title, s.description, s.expires, s.created_at,
               p.handle, p.display_name, p.avatar_cid,
               e.blob_cid as emoji_blob_cid, e.mime_type, e.emoji_name, e.alt_text, e.did as emoji_did
        FROM statuses s
        LEFT JOIN profiles p ON s.did = p.did
        LEFT JOIN emojis e ON s.emoji_ref = 'at://' || e.at
        WHERE (s.expires IS NULL OR datetime(s.expires) > datetime('now'))
        ORDER BY s.created_at DESC
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    use jacquard_common::types::string::{Datetime, Did, Handle};
    use lexicons::vg_nat::istat::status::list_statuses::StatusView;

    let statuses: Vec<_> = rows
        .iter()
        .filter_map(|row| {
            let did: String = row.try_get("did").ok()?;
            let rkey: String = row.try_get("rkey").ok()?;
            let emoji_ref: String = row.try_get("emoji_ref").ok()?;
            let emoji_blob_cid: Option<String> = row.try_get("emoji_blob_cid").ok().flatten();
            let title: Option<String> = row
                .try_get("title")
                .ok()
                .and_then(|s: String| if s.is_empty() { None } else { Some(s) });
            let description: Option<String> = row
                .try_get("description")
                .ok()
                .and_then(|s: String| if s.is_empty() { None } else { Some(s) });
            let expires: Option<String> = row.try_get("expires").ok();
            let created_at: String = row.try_get("created_at").ok()?;
            let handle: Option<String> = row.try_get("handle").ok().flatten();
            let display_name: Option<String> = row.try_get("display_name").ok().flatten();
            let avatar_cid: Option<String> = row.try_get("avatar_cid").ok().flatten();
            let emoji_name: Option<String> = row.try_get("emoji_name").ok().flatten();
            let alt_text: Option<String> = row.try_get("alt_text").ok().flatten();
            let emoji_did: Option<String> = row.try_get("emoji_did").ok().flatten();

            let mime: Option<String> = row.try_get("mime_type").ok().flatten();

            // Helper to get file extension from mime type
            let mime_ext = mime
                .as_deref()
                .and_then(|m| match m {
                    "image/png" => Some("png"),
                    "image/jpeg" => Some("jpeg"),
                    "image/jpg" => Some("jpeg"),
                    "image/webp" => Some("webp"),
                    "image/gif" => Some("gif"),
                    _ => Some("jpeg"), // default fallback
                })
                .unwrap_or("jpeg");

            // If we have the emoji blob CID from our DB, use it
            // Otherwise try to extract from the emoji_ref AT-URI
            let emoji_url = if let Some(blob_cid) = emoji_blob_cid {
                if let Some(emoji_owner_did) = emoji_did {
                    format!(
                        "https://at.uwu.wang/{}/{}@{}",
                        emoji_owner_did, blob_cid, mime_ext
                    )
                } else {
                    // fallback: try to extract DID from emoji_ref
                    emoji_ref
                        .strip_prefix("at://")
                        .and_then(|s| s.split('/').next())
                        .map(|emoji_owner| {
                            format!(
                                "https://at.uwu.wang/{}/{}@{}",
                                emoji_owner, blob_cid, mime_ext
                            )
                        })
                        .unwrap_or_else(|| {
                            format!("https://at.uwu.wang/{}/{}@{}", did, blob_cid, mime_ext)
                        })
                }
            } else {
                // Fallback: try to extract CID from emoji_ref AT-URI (last segment)
                // This won't work if we don't have the emoji indexed, but at least won't crash
                emoji_ref
                    .split('/')
                    .last()
                    .map(|cid| format!("https://at.uwu.wang/{}/{}@{}", did, cid, mime_ext))
                    .unwrap_or_else(|| {
                        eprintln!(
                            "Warning: emoji not found for status {}, emoji_ref: {}",
                            rkey, emoji_ref
                        );
                        String::new()
                    })
            };

            let avatar_url =
                avatar_cid.map(|cid| format!("https://at.uwu.wang/{}/{}@webp", did, cid));

            let handle_str = handle.unwrap_or(did.clone());

            // Validate datetime format before passing to raw_str to avoid panics
            // Skip statuses with invalid datetimes
            if created_at.is_empty() || !created_at.contains('T') {
                eprintln!("Invalid created_at datetime for status: {}", created_at);
                return None;
            }

            Some(
                StatusView::new()
                    .did(Did::from_str(&did).ok()?)
                    .handle(Handle::from_str(&handle_str).ok()?)
                    .maybe_display_name(display_name.map(Into::into))
                    .maybe_avatar_url(avatar_url.map(Into::into))
                    .rkey(rkey)
                    .emoji_url(emoji_url)
                    .maybe_emoji_name(emoji_name.map(Into::into))
                    .maybe_emoji_alt(alt_text.map(Into::into))
                    .maybe_title(title.map(Into::into))
                    .maybe_description(description.map(Into::into))
                    .maybe_expires(
                        expires
                            .filter(|e| !e.is_empty() && e.contains('T'))
                            .map(|e| Datetime::raw_str(e)),
                    )
                    .created_at(Datetime::raw_str(created_at))
                    .build(),
            )
        })
        .collect();

    let output = ListStatusesOutput {
        statuses,
        cursor: None,
        extra_data: None,
    };

    Ok(Json(output))
}
