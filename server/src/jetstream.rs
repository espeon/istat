use anyhow::Result;
use async_trait::async_trait;
use rocketman::{
    connection::JetstreamConnection, handler, ingestion::LexiconIngestor,
    options::JetstreamOptions, types::event::Event,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct EmojiRecord {
    emoji: BlobRef,
    alt_text: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct BlobRef {
    #[serde(rename = "$link")]
    link: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusRecord {
    emoji: StrongRef,
    title: Option<String>,
    description: Option<String>,
    expires: Option<String>,
    created_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
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

        let rkey = &commit.rkey;
        let operation = &commit.operation;

        match operation {
            rocketman::types::event::Operation::Create
            | rocketman::types::event::Operation::Update => {
                let record: EmojiRecord = serde_json::from_value(
                    commit
                        .record
                        .ok_or_else(|| anyhow::anyhow!("Missing record"))?,
                )?;
                let created_at = chrono::Utc::now().to_rfc3339();
                let at_uri = format!("{}/vg.nat.istat.moji.emoji/{}", event.did, rkey);

                sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO emojis (at, did, blob_cid, alt_text, created_at)
                    VALUES (?, ?, ?, ?, ?)
                    "#,
                )
                .bind(&at_uri)
                .bind(&event.did)
                .bind(&record.emoji.link)
                .bind(&record.alt_text)
                .bind(&created_at)
                .execute(&self.db)
                .await?;

                println!(
                    "Inserted/updated emoji: at={}, cid={}",
                    at_uri, record.emoji.link
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
                let record: StatusRecord = serde_json::from_value(
                    commit
                        .record
                        .ok_or_else(|| anyhow::anyhow!("Missing record"))?,
                )?;
                let at_uri = format!("{}/vg.nat.istat.status.record/{}", event.did, rkey);

                sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO statuses (at, did, rkey, emoji_ref, emoji_ref_cid, title, description, expires, created_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(&at_uri)
                .bind(&event.did)
                .bind(rkey)
                .bind(&record.emoji.uri)
                .bind(&record.emoji.cid)
                .bind(&record.title)
                .bind(&record.description)
                .bind(&record.expires)
                .bind(&record.created_at)
                .execute(&self.db)
                .await?;

                println!(
                    "Inserted/updated status: at={}, emoji={}",
                    at_uri, record.emoji.uri
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

pub async fn start_jetstream(db: SqlitePool) -> Result<()> {
    let opts = JetstreamOptions::builder()
        .wanted_collections(vec![
            "vg.nat.istat.moji.emoji".to_string(),
            "vg.nat.istat.status.record".to_string(),
        ])
        .build();

    let jetstream = JetstreamConnection::new(opts);

    let mut ingestors: HashMap<String, Box<dyn LexiconIngestor + Send + Sync>> = HashMap::new();
    ingestors.insert(
        "vg.nat.istat.moji.emoji".to_string(),
        Box::new(EmojiIngestor::new(db.clone())),
    );
    ingestors.insert(
        "vg.nat.istat.status.record".to_string(),
        Box::new(StatusIngestor::new(db)),
    );

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
