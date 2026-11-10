use std::path::{Path, PathBuf};

use anyhow::Context;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Secret record stored in the database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Secret {
    pub id: i64,
    pub project: String,
    pub scope: String,
    pub key: String,
    pub value_blob: Vec<u8>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Initialize SQLite database schema and enable WAL mode.
pub fn init_db(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch("PRAGMA journal_mode = WAL;")
        .context("failed to enable WAL mode")?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .context("failed to enable foreign keys")?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS secrets (
            id          INTEGER PRIMARY KEY,
            project     TEXT NOT NULL,
            scope       TEXT NOT NULL DEFAULT 'default',
            key         TEXT NOT NULL,
            value_blob  BLOB NOT NULL,
            created_at  INTEGER NOT NULL,
            updated_at  INTEGER NOT NULL,
            UNIQUE(project, scope, key)
        );

        CREATE TABLE IF NOT EXISTS audit_log (
            id          INTEGER PRIMARY KEY,
            project     TEXT NOT NULL,
            scope       TEXT NOT NULL,
            key         TEXT NOT NULL,
            action      TEXT NOT NULL,
            at          INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS vault_meta (
            id          INTEGER PRIMARY KEY CHECK (id = 1),
            salt        BLOB NOT NULL,
            created_at  INTEGER NOT NULL
        );
        "#,
    )
    .context("failed to initialize vault database schema")?;

    Ok(())
}

/// Returns the default vault database path: `~/.config/envault/vault.db`.
pub fn default_db_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("failed to determine user home directory")?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("envault")
        .join("vault.db"))
}

/// Open or create a vault database at the specified path and initialize its schema.
pub fn open_db(path: impl AsRef<Path>) -> anyhow::Result<Connection> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create database directory at {}", parent.display()))?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("failed to open database at {}", path.display()))?;
    init_db(&conn)?;
    Ok(conn)
}

/// Open an in-memory database and initialize its schema (useful for tests).
pub fn open_in_memory() -> anyhow::Result<Connection> {
    let conn = Connection::open_in_memory()
        .context("failed to open in-memory SQLite database")?;
    init_db(&conn)?;
    Ok(conn)
}

/// Retrieve the master salt stored in vault_meta table, if present.
pub fn get_vault_salt(conn: &Connection) -> anyhow::Result<Option<Vec<u8>>> {
    let mut stmt = conn
        .prepare("SELECT salt FROM vault_meta WHERE id = 1")
        .context("failed to prepare vault_meta query")?;

    let mut rows = stmt.query([]).context("failed to query vault_meta")?;
    if let Some(row) = rows.next().context("failed to read vault_meta row")? {
        let salt: Vec<u8> = row.get(0).context("failed to read salt column")?;
        Ok(Some(salt))
    } else {
        Ok(None)
    }
}

/// Save the master salt in vault_meta table.
pub fn set_vault_salt(conn: &Connection, salt: &[u8]) -> anyhow::Result<()> {
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        r#"
        INSERT INTO vault_meta (id, salt, created_at)
        VALUES (1, ?1, ?2)
        ON CONFLICT(id) DO UPDATE SET salt = excluded.salt;
        "#,
        rusqlite::params![salt, now],
    )
    .context("failed to insert or update vault_meta salt")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_db_creates_tables() {
        let conn = open_in_memory().expect("in-memory db open should succeed");

        // Verify tables exist in sqlite_master
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(tables.contains(&"secrets".to_string()));
        assert!(tables.contains(&"audit_log".to_string()));
        assert!(tables.contains(&"vault_meta".to_string()));
    }

    #[test]
    fn test_init_db_idempotent() {
        let conn = open_in_memory().expect("in-memory db open should succeed");
        // Running init_db again should not fail
        init_db(&conn).expect("second init_db call should succeed without error");
    }

    #[test]
    fn test_secrets_unique_constraint() {
        let conn = open_in_memory().expect("open should succeed");
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO secrets (project, scope, key, value_blob, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["proj", "default", "API_KEY", vec![1u8, 2, 3], now, now],
        ).unwrap();

        // Inserting identical (project, scope, key) should fail UNIQUE constraint
        let duplicate_insert = conn.execute(
            "INSERT INTO secrets (project, scope, key, value_blob, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["proj", "default", "API_KEY", vec![4u8, 5, 6], now, now],
        );
        assert!(duplicate_insert.is_err(), "duplicate secret must violate UNIQUE constraint");
    }

    #[test]
    fn test_vault_meta_salt_persistence() {
        let conn = open_in_memory().expect("open should succeed");

        let initial_salt = get_vault_salt(&conn).unwrap();
        assert_eq!(initial_salt, None);

        let test_salt = vec![17u8; 32];
        set_vault_salt(&conn, &test_salt).unwrap();

        let retrieved = get_vault_salt(&conn).unwrap();
        assert_eq!(retrieved, Some(test_salt.clone()));

        // Update salt
        let updated_salt = vec![19u8; 32];
        set_vault_salt(&conn, &updated_salt).unwrap();
        let retrieved_updated = get_vault_salt(&conn).unwrap();
        assert_eq!(retrieved_updated, Some(updated_salt));

        // Ensure CHECK (id = 1) constraint is enforced
        let invalid_insert = conn.execute(
            "INSERT INTO vault_meta (id, salt, created_at) VALUES (2, ?1, 1000)",
            rusqlite::params![vec![0u8; 32]],
        );
        assert!(invalid_insert.is_err(), "vault_meta id != 1 must fail CHECK constraint");
    }

    #[test]
    fn test_wal_mode_on_file_db() {
        let temp_dir = std::env::temp_dir().join(format!("envault_test_{}", rand::random::<u64>()));
        let db_path = temp_dir.join("test_vault.db");

        let conn = open_db(&db_path).expect("open_db should succeed");

        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("query journal_mode should succeed");

        assert_eq!(mode.to_lowercase(), "wal");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
