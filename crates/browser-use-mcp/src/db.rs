use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Arc<Self>> {
        let conn = Connection::open(path.as_ref())
            .with_context(|| format!("open SQLite at {}", path.as_ref().display()))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS users (
                 user_id    TEXT PRIMARY KEY,
                 key_hash   TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 last_seen  INTEGER,
                 active     INTEGER NOT NULL DEFAULT 1
             );",
        )
        .context("initialise users table")?;
        Ok(Arc::new(Self {
            conn: Mutex::new(conn),
        }))
    }

    /// Create a new user and return the raw API key (shown once, never stored).
    pub fn create_user(&self, user_id: &str) -> Result<String> {
        let raw_key = generate_api_key(user_id);
        let hash = hash_key(&raw_key);
        let now = unix_now();
        self.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO users (user_id, key_hash, created_at) VALUES (?1, ?2, ?3)",
                params![user_id, hash, now],
            )
            .with_context(|| format!("create user {user_id}"))?;
        Ok(raw_key)
    }

    /// Validate a raw API key. Returns the user_id if valid and active, updates last_seen.
    pub fn validate_key(&self, raw_key: &str) -> Result<Option<String>> {
        let hash = hash_key(raw_key);
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT user_id FROM users WHERE key_hash = ?1 AND active = 1",
            params![hash],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(user_id) => {
                let _ = conn.execute(
                    "UPDATE users SET last_seen = ?1 WHERE user_id = ?2",
                    params![unix_now(), &user_id],
                );
                Ok(Some(user_id))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_users(&self) -> Result<Vec<UserRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT user_id, created_at, last_seen, active
             FROM users ORDER BY created_at DESC",
        )?;
        let users = stmt
            .query_map([], |row| {
                Ok(UserRecord {
                    user_id: row.get(0)?,
                    created_at: row.get(1)?,
                    last_seen: row.get(2)?,
                    active: row.get::<_, i64>(3)? != 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(users)
    }

    /// Soft-delete: sets active=0 but keeps the Chrome profile on disk.
    pub fn revoke_user(&self, user_id: &str) -> Result<bool> {
        let n = self
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE users SET active = 0 WHERE user_id = ?1",
                params![user_id],
            )
            .context("revoke user")?;
        Ok(n > 0)
    }

    /// Issue a new key for an existing user (re-activates if revoked).
    pub fn rotate_key(&self, user_id: &str) -> Result<Option<String>> {
        let raw_key = generate_api_key(user_id);
        let hash = hash_key(&raw_key);
        let n = self
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE users SET key_hash = ?1, active = 1 WHERE user_id = ?2",
                params![hash, user_id],
            )
            .context("rotate key")?;
        if n > 0 {
            Ok(Some(raw_key))
        } else {
            Ok(None)
        }
    }

    pub fn user_exists(&self, user_id: &str) -> bool {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT 1 FROM users WHERE user_id = ?1",
                params![user_id],
                |_| Ok(()),
            )
            .is_ok()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UserRecord {
    pub user_id: String,
    pub created_at: i64,
    pub last_seen: Option<i64>,
    pub active: bool,
}

fn hash_key(raw: &str) -> String {
    let mut h = Sha256::new();
    h.update(raw.as_bytes());
    format!("{:x}", h.finalize())
}

fn generate_api_key(user_id: &str) -> String {
    let random = Uuid::new_v4().simple().to_string();
    let slug: String = user_id
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(8)
        .collect::<String>()
        .to_lowercase();
    if slug.is_empty() {
        format!("buak_{random}")
    } else {
        format!("buak_{slug}_{random}")
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
