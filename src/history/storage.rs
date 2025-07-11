use super::Message;
use crate::{Error, Result};
use libsql::{Builder, Database};
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};

pub struct HistoryStorage {
    db: Option<Database>,
    // In-memory fallback storage
    fallback: Arc<Mutex<Vec<Message>>>,
}

impl HistoryStorage {
    pub async fn new(db_path: &str) -> Result<Self> {
        let mut storage = Self {
            db: None,
            fallback: Arc::new(Mutex::new(Vec::new())),
        };

        // Try to initialize database
        match storage.init_database(db_path).await {
            Ok(()) => {
                info!("Database initialized successfully: {}", db_path);
            }
            Err(e) => {
                warn!(
                    "Database initialization failed, using in-memory fallback: {}",
                    e
                );
            }
        }

        Ok(storage)
    }

    async fn init_database(&mut self, db_path: &str) -> Result<()> {
        // Handle in-memory database
        let db = if db_path == ":memory:" {
            Builder::new_local(":memory:").build().await?
        } else {
            Builder::new_local(db_path).build().await?
        };

        // Create table if it doesn't exist
        let conn = db.connect()?;
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at DATETIME NOT NULL
            )
            "#,
            (),
        )
        .await?;

        self.db = Some(db);
        Ok(())
    }

    pub async fn save(&self, message: Message) -> Result<()> {
        // Try database first
        if let Some(ref db) = self.db {
            match self.save_to_db(db, &message).await {
                Ok(()) => {
                    debug!("Message saved to database: {}", message.session_id);
                    return Ok(());
                }
                Err(e) => {
                    warn!("Failed to save to database, using fallback: {}", e);
                }
            }
        }

        // Fallback to in-memory storage
        self.save_to_fallback(message)?;
        Ok(())
    }

    async fn save_to_db(&self, db: &Database, message: &Message) -> Result<()> {
        let conn = db.connect()?;
        conn.execute(
            "INSERT INTO messages (session_id, role, content, created_at) VALUES (?, ?, ?, ?)",
            (
                message.session_id.as_str(),
                message.role.as_str(),
                message.content.as_str(),
                message.created_at.to_rfc3339(),
            ),
        )
        .await?;
        Ok(())
    }

    fn save_to_fallback(&self, message: Message) -> Result<()> {
        let mut fallback = self
            .fallback
            .lock()
            .map_err(|e| Error::internal(format!("Mutex lock failed: {}", e)))?;
        fallback.push(message);
        Ok(())
    }

    pub async fn list(&self, session_id: &str) -> Result<Vec<Message>> {
        // Try database first
        if let Some(ref db) = self.db {
            match self.list_from_db(db, session_id).await {
                Ok(messages) => {
                    debug!(
                        "Retrieved {} messages from database for session: {}",
                        messages.len(),
                        session_id
                    );
                    return Ok(messages);
                }
                Err(e) => {
                    warn!("Failed to read from database, using fallback: {}", e);
                }
            }
        }

        // Fallback to in-memory storage
        self.list_from_fallback(session_id)
    }

    async fn list_from_db(&self, db: &Database, session_id: &str) -> Result<Vec<Message>> {
        let conn = db.connect()?;
        let mut rows = conn.query(
            "SELECT id, session_id, role, content, created_at FROM messages WHERE session_id = ? ORDER BY id ASC",
            [session_id]
        ).await?;

        let mut messages = Vec::new();
        while let Some(row) = rows.next().await? {
            let created_at_str: String = row.get(4)?;
            let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .map_err(|e| Error::internal(format!("Failed to parse timestamp: {}", e)))?
                .with_timezone(&chrono::Utc);

            let message = Message {
                id: Some(row.get(0)?),
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at,
            };
            messages.push(message);
        }

        Ok(messages)
    }

    fn list_from_fallback(&self, session_id: &str) -> Result<Vec<Message>> {
        let fallback = self
            .fallback
            .lock()
            .map_err(|e| Error::internal(format!("Mutex lock failed: {}", e)))?;

        let messages: Vec<Message> = fallback
            .iter()
            .filter(|msg| msg.session_id == session_id)
            .cloned()
            .collect();

        debug!(
            "Retrieved {} messages from fallback for session: {}",
            messages.len(),
            session_id
        );
        Ok(messages)
    }
}
