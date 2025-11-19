use anyhow::Result;
use async_trait::async_trait;
use jacquard::types::value;
use lexicons::vg_nat::istat::moji::emoji::Emoji;

use lexicons::{app_bsky::actor::profile::Profile, vg_nat::istat::status};
use rocketman::{
    connection::JetstreamConnection,
    handler::{self, Ingestors},
    ingestion::LexiconIngestor,
    options::JetstreamOptions,
    types::event::Event,
};
use serde::Deserialize;
use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::{Arc, Mutex};

/// Hydrates a profile from the network if it doesn't exist in the database.
/// Returns the profile data (whether it was freshly fetched or already existed).
async fn hydrate_profile(db: &SqlitePool, did: &str) -> Result<Option<serde_json::Value>> {
    // Check if profile already exists
    let existing_profile: Option<String> = sqlx::query_scalar(
        "SELECT json_object('did', did, 'handle', handle, 'display_name', display_name, 'description', description, 'avatar_cid', avatar_cid, 'banner_cid', banner_cid, 'pronouns', pronouns, 'website', website, 'created_at', created_at) FROM profiles WHERE did = ?"
    )
    .bind(did)
    .fetch_optional(db)
    .await?;

    if let Some(profile_json) = existing_profile {
        return Ok(serde_json::from_str(&profile_json).ok());
    }

    eprintln!("Hydrating profile for {}", did);

    // Fetch handle from PLC directory
    let handle_url = format!("https://plc.directory/{}", did);
    let handle = if let Ok(resp) = reqwest::get(&handle_url).await {
        if resp.status().is_success() {
            if let Ok(did_doc) = resp.json::<serde_json::Value>().await {
                did_doc
                    .get("alsoKnownAs")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.strip_prefix("at://"))
                    .map(|s| s.to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Fetch profile from Bluesky API
    let profile_url = format!(
        "https://public.api.bsky.app/xrpc/com.atproto.repo.getRecord?repo={}&collection=app.bsky.actor.profile&rkey=self",
        did
    );

    if let Ok(resp) = reqwest::get(&profile_url).await {
        if resp.status().is_success() {
            if let Ok(profile_data) = resp.json::<serde_json::Value>().await {
                if let Some(record) = profile_data.get("value") {
                    let now = chrono::Utc::now().to_rfc3339();
                    let display_name = record.get("displayName").and_then(|v| v.as_str());
                    let description = record.get("description").and_then(|v| v.as_str());
                    let pronouns = record.get("pronouns").and_then(|v| v.as_str());
                    let website = record.get("website").and_then(|v| v.as_str());
                    let created_at = record.get("createdAt").and_then(|v| v.as_str());
                    let avatar_cid = record
                        .get("avatar")
                        .and_then(|v| v.get("ref"))
                        .and_then(|v| v.get("$link"))
                        .and_then(|v| v.as_str());
                    let banner_cid = record
                        .get("banner")
                        .and_then(|v| v.get("ref"))
                        .and_then(|v| v.get("$link"))
                        .and_then(|v| v.as_str());

                    sqlx::query(
                        r#"
                        INSERT INTO profiles (did, handle, display_name, description, avatar_cid, banner_cid, pronouns, website, created_at, updated_at, account_status, last_seen_at)
                        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?)
                        "#,
                    )
                    .bind(did)
                    .bind(handle.as_deref())
                    .bind(display_name)
                    .bind(description)
                    .bind(avatar_cid)
                    .bind(banner_cid)
                    .bind(pronouns)
                    .bind(website)
                    .bind(created_at)
                    .bind(&now)
                    .bind(&now)
                    .execute(db)
                    .await?;

                    eprintln!(
                        "Hydrated profile for {} (@{})",
                        did,
                        handle.as_deref().unwrap_or("unknown")
                    );

                    // Return the newly created profile
                    return Ok(Some(serde_json::json!({
                        "did": did,
                        "handle": handle,
                        "display_name": display_name,
                        "description": description,
                        "avatar_cid": avatar_cid,
                        "banner_cid": banner_cid,
                        "pronouns": pronouns,
                        "website": website,
                        "created_at": created_at,
                    })));
                }
            }
        }
    }

    Ok(None)
}

/// Helper struct to deserialize strongRef from Data
#[derive(Debug, Deserialize)]
struct StrongRef {
    uri: String,
    cid: String,
}

pub struct EmojiIngestor {
    db: SqlitePool,
}

impl EmojiIngestor {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl LexiconIngestor for EmojiIngestor {
    async fn ingest(&self, event: Event<Value>) -> Result<()> {
        let commit = match event.commit {
            Some(c) => c,
            None => return Ok(()),
        };

        let record = match commit.record {
            Some(r) => value::from_json_value::<Emoji>(r)?,
            None => return Ok(()),
        };

        let rkey = &commit.rkey;
        let operation = &commit.operation;

        match operation {
            rocketman::types::event::Operation::Create
            | rocketman::types::event::Operation::Update => {
                let created_at = chrono::Utc::now().to_rfc3339();
                let at_uri = format!("{}/vg.nat.istat.moji.emoji/{}", event.did, rkey);

                // Hydrate profile for this user if we don't have it
                let _ = hydrate_profile(&self.db, &event.did).await;

                let blob = record.emoji.blob();
                let cid = blob.r#ref.as_str();
                let mime_type = blob.mime_type.as_str();

                sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO emojis (at, did, blob_cid, mime_type, emoji_name, alt_text, created_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(&at_uri)
                .bind(&event.did)
                .bind(cid)
                .bind(mime_type)
                .bind(&record.name.to_string())
                .bind(&record.alt_text.map(|s| s.to_string()))
                .bind(&created_at)
                .execute(&self.db)
                .await?;

                println!(
                    "Inserted/updated emoji: at={}, name={:?}, cid={:?}, mime={}",
                    at_uri, record.name, cid, mime_type
                );
            }
            rocketman::types::event::Operation::Delete => {
                let at_uri = format!("{}/vg.nat.istat.moji.emoji/{}", event.did, rkey);

                sqlx::query(
                    r#"
                    DELETE FROM emojis WHERE at = ?
                    "#,
                )
                .bind(&at_uri)
                .execute(&self.db)
                .await?;

                println!("Deleted emoji: at={}", at_uri);
            }
        }

        Ok(())
    }
}

pub struct StatusIngestor {
    db: SqlitePool,
}

impl StatusIngestor {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl LexiconIngestor for StatusIngestor {
    async fn ingest(&self, event: Event<Value>) -> Result<()> {
        let commit = match event.commit {
            Some(c) => c,
            None => return Ok(()),
        };

        let rkey = &commit.rkey;
        let operation = &commit.operation;

        match operation {
            rocketman::types::event::Operation::Create
            | rocketman::types::event::Operation::Update => {
                let record = value::from_json_value::<status::record::Record>(
                    commit
                        .record
                        .ok_or_else(|| anyhow::anyhow!("Missing record"))?,
                )?;
                let at_uri = format!("{}/vg.nat.istat.status.record/{}", event.did, rkey);

                // Hydrate profile for this user if we don't have it
                let _ = hydrate_profile(&self.db, &event.did).await;

                // Extract uri and cid from the emoji strongRef (which is a Data type)
                // Deserialize Data as StrongRef
                let emoji_ref: StrongRef = value::from_data(&record.emoji)?;

                sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO statuses (at, did, rkey, emoji_ref, emoji_ref_cid, title, description, expires, created_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(&at_uri)
                .bind(&event.did)
                .bind(rkey)
                .bind(&emoji_ref.uri)
                .bind(&emoji_ref.cid)
                .bind(&record.title.as_ref().map(|s| s.as_ref()))
                .bind(&record.description.as_ref().map(|s| s.as_ref()))
                .bind(&record.expires.as_ref().map(|dt| dt.as_str()))
                .bind(record.created_at.as_str())
                .execute(&self.db)
                .await?;

                println!(
                    "Inserted/updated status: at={}, emoji={}",
                    at_uri, emoji_ref.uri
                );
            }
            rocketman::types::event::Operation::Delete => {
                let at_uri = format!("{}/vg.nat.istat.status.record/{}", event.did, rkey);

                sqlx::query(
                    r#"
                    DELETE FROM statuses WHERE at = ?
                    "#,
                )
                .bind(&at_uri)
                .execute(&self.db)
                .await?;

                println!("Deleted status: at={}", at_uri);
            }
        }

        Ok(())
    }
}

pub struct ProfileIngestor {
    db: SqlitePool,
}

impl ProfileIngestor {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl LexiconIngestor for ProfileIngestor {
    async fn ingest(&self, event: Event<Value>) -> Result<()> {
        let commit = match event.commit {
            Some(c) => c,
            None => return Ok(()),
        };

        let operation = &commit.operation;

        match operation {
            rocketman::types::event::Operation::Create
            | rocketman::types::event::Operation::Update => {
                let record: Profile = value::from_json_value::<Profile>(
                    commit
                        .record
                        .ok_or_else(|| anyhow::anyhow!("Missing record"))?,
                )?;

                let updated_at = chrono::Utc::now().to_rfc3339();

                // Only update profiles that already exist in the database
                let result = sqlx::query(
                    r#"
                    UPDATE profiles
                    SET display_name = ?,
                        description = ?,
                        avatar_cid = ?,
                        banner_cid = ?,
                        pronouns = ?,
                        website = ?,
                        created_at = COALESCE(?, created_at),
                        updated_at = ?,
                        last_seen_at = ?
                    WHERE did = ?
                    "#,
                )
                .bind(record.display_name.as_ref().map(|s| s.as_ref()))
                .bind(record.description.as_ref().map(|s| s.as_ref()))
                .bind(record.avatar.as_ref().map(|b| b.blob().r#ref.as_str()))
                .bind(record.banner.as_ref().map(|b| b.blob().r#ref.as_str()))
                .bind(record.pronouns.as_ref().map(|s| s.as_ref()))
                .bind(record.website.as_ref().map(|u| u.as_str()))
                .bind(record.created_at.as_ref().map(|dt| dt.as_str()))
                .bind(&updated_at)
                .bind(&updated_at)
                .bind(&event.did)
                .execute(&self.db)
                .await?;

                if result.rows_affected() > 0 {
                    println!("Updated profile: did={}", event.did);
                }
            }
            rocketman::types::event::Operation::Delete => {
                // Mark as deleted instead of removing
                let now = chrono::Utc::now().to_rfc3339();
                sqlx::query(
                    r#"
                    UPDATE profiles
                    SET account_status = 'deleted',
                        account_status_updated_at = ?
                    WHERE did = ?
                    "#,
                )
                .bind(&now)
                .bind(&event.did)
                .execute(&self.db)
                .await?;

                println!("Marked profile as deleted: did={}", event.did);
            }
        }

        Ok(())
    }
}

pub struct IdentityIngestor {
    db: SqlitePool,
}

impl IdentityIngestor {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl LexiconIngestor for IdentityIngestor {
    async fn ingest(&self, event: Event<Value>) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        // Handle identity events (handle changes)
        if let Some(identity) = event.identity {
            let did = &identity.did;

            if let Some(handle) = identity.handle {
                // Only update if profile already exists
                let result = sqlx::query(
                    r#"
                    UPDATE profiles
                    SET handle = ?,
                        updated_at = ?,
                        last_seen_at = ?
                    WHERE did = ?
                    "#,
                )
                .bind(&handle)
                .bind(&now)
                .bind(&now)
                .bind(did)
                .execute(&self.db)
                .await?;

                if result.rows_affected() > 0 {
                    println!("Updated handle for did={}: {}", did, handle);
                }
            }
        }

        Ok(())
    }
}

pub struct AccountIngestor {
    db: SqlitePool,
}

impl AccountIngestor {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl LexiconIngestor for AccountIngestor {
    async fn ingest(&self, event: Event<Value>) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        // Handle account status events (active/inactive/deleted/suspended)
        if let Some(account) = event.account {
            let did = &account.did;

            // Map account status enum to string
            let account_status = if let Some(status) = account.status {
                match status {
                    rocketman::types::event::AccountStatus::Activated => "active",
                    rocketman::types::event::AccountStatus::TakenDown => "suspended",
                    rocketman::types::event::AccountStatus::Suspended => "suspended",
                    rocketman::types::event::AccountStatus::Deleted => "deleted",
                    rocketman::types::event::AccountStatus::Deactivated => "deactivated",
                }
            } else {
                "active"
            };

            // Only update if profile already exists
            let result = sqlx::query(
                r#"
                UPDATE profiles
                SET account_status = ?,
                    account_status_updated_at = ?,
                    last_seen_at = ?
                WHERE did = ?
                "#,
            )
            .bind(account_status)
            .bind(&now)
            .bind(&now)
            .bind(did)
            .execute(&self.db)
            .await?;

            if result.rows_affected() > 0 {
                println!("Updated account status for did={}: {}", did, account_status);
            }
        }

        Ok(())
    }
}

pub async fn start_jetstream(db: SqlitePool) -> Result<()> {
    let opts = JetstreamOptions::builder()
        .ws_url(rocketman::endpoints::JetstreamEndpoints::Public(
            rocketman::endpoints::JetstreamEndpointLocations::UsEast,
            1,
        ))
        .wanted_collections(vec![
            "app.bsky.actor.profile".to_string(),
            "vg.nat.istat.moji.emoji".to_string(),
            "vg.nat.istat.status.record".to_string(),
        ])
        .bound(8 * 8 * 8 * 8 * 8 * 8) // 262144
        .build();

    let jetstream = JetstreamConnection::new(opts);

    let mut ingestors: Ingestors = Ingestors::new();
    ingestors.commits.insert(
        "vg.nat.istat.moji.emoji".to_string(),
        Box::new(EmojiIngestor::new(db.clone())),
    );
    ingestors.commits.insert(
        "vg.nat.istat.status.record".to_string(),
        Box::new(StatusIngestor::new(db.clone())),
    );
    ingestors.commits.insert(
        "app.bsky.actor.profile".to_string(),
        Box::new(ProfileIngestor::new(db.clone())),
    );
    ingestors.identity = Some(Box::new(IdentityIngestor::new(db.clone())));
    ingestors.account = Some(Box::new(AccountIngestor::new(db)));

    let cursor: Arc<Mutex<Option<u64>>> = Arc::new(Mutex::new(None));

    let msg_rx = jetstream.get_msg_rx();
    let reconnect_tx = jetstream.get_reconnect_tx();

    let c_cursor = cursor.clone();
    tokio::spawn(async move {
        while let Ok(message) = msg_rx.recv_async().await {
            if let Err(e) =
                handler::handle_message(message, &ingestors, reconnect_tx.clone(), c_cursor.clone())
                    .await
            {
                eprintln!("Error processing message: {}", e);
            }
        }
    });

    if let Err(e) = jetstream.connect(cursor.clone()).await {
        eprintln!("Failed to connect to Jetstream: {}", e);
        return Err(anyhow::anyhow!("Jetstream connection failed"));
    }

    Ok(())
}
