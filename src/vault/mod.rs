use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;

pub mod audit;
pub mod crypto;
pub mod db;
pub mod kdf;
pub mod keychain;

// Re-exports of key structs and functions
pub use audit::{get_audit_log, record_audit, record_audit_at, AuditAction, AuditEntry};
pub use crypto::{decrypt, decrypt_payload, encrypt, encrypt_payload, EncryptedPayload};
pub use db::{
    default_db_path, get_vault_salt, init_db, open_db, open_in_memory, set_vault_salt, Secret,
};
pub use kdf::{derive_key, generate_salt};
pub use keychain::{clear_fallback_keychain, delete_key, load_key, store_key};

/// High-level client for managing encrypted project secrets in a SQLite vault.
pub struct VaultClient {
    conn: rusqlite::Connection,
    project: String,
    scope: String,
    key: [u8; 32],
}

impl std::fmt::Debug for VaultClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VaultClient")
            .field("project", &self.project)
            .field("scope", &self.scope)
            .field("key", &"[REDACTED]")
            .finish()
    }
}

impl VaultClient {
    /// Open a SQLite database at `db_path`, initialize schemas, ensure master salt exists,
    /// and resolve the 32-byte encryption key from the keychain or by deriving it from `password`.
    pub fn open(
        db_path: &Path,
        project: &str,
        scope: &str,
        password: &str,
    ) -> anyhow::Result<Self> {
        let conn = db::open_db(db_path)?;

        // Check vault_meta for salt. If not present, generate salt and store it with created_at.
        let salt = match db::get_vault_salt(&conn)? {
            Some(s) => {
                if s.len() != 32 {
                    anyhow::bail!(
                        "invalid salt length in vault_meta: expected 32 bytes, got {}",
                        s.len()
                    );
                }
                let mut salt_arr = [0u8; 32];
                salt_arr.copy_from_slice(&s);
                salt_arr
            }
            None => {
                let salt_arr = kdf::generate_salt();
                db::set_vault_salt(&conn, &salt_arr)?;
                salt_arr
            }
        };

        // Try loading cached key from keychain. If not present, derive key from password and salt
        // using kdf::derive_key, and attempt to cache it in keychain.
        let key = match keychain::load_key(project)? {
            Some(k) => k,
            None => {
                let derived = kdf::derive_key(password, &salt)?;
                let _ = keychain::store_key(project, &derived);
                derived
            }
        };

        Self::from_connection(conn, project, scope, key)
    }

    /// Construct a VaultClient from an existing rusqlite connection and encryption key.
    /// Useful for in-memory and test setups. Initializes the database schema if needed.
    pub fn from_connection(
        conn: rusqlite::Connection,
        project: &str,
        scope: &str,
        key: [u8; 32],
    ) -> anyhow::Result<Self> {
        db::init_db(&conn)?;
        Ok(Self {
            conn,
            project: project.to_string(),
            scope: scope.to_string(),
            key,
        })
    }

    /// Open an in-memory VaultClient with a generated salt and derived key (useful in tests).
    pub fn open_in_memory(project: &str, scope: &str, password: &str) -> anyhow::Result<Self> {
        let conn = db::open_in_memory()?;
        let salt = kdf::generate_salt();
        db::set_vault_salt(&conn, &salt)?;
        let key = kdf::derive_key(password, &salt)?;
        Self::from_connection(conn, project, scope, key)
    }

    /// Get current project name.
    pub fn project(&self) -> &str {
        &self.project
    }

    /// Get current scope name.
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// Get encryption key reference.
    pub fn key(&self) -> &[u8; 32] {
        &self.key
    }

    /// Get reference to underlying SQLite connection.
    pub fn connection(&self) -> &rusqlite::Connection {
        &self.conn
    }

    /// Encrypts `value` with `crypto::encrypt` using `self.key`, upserts into the `secrets` table,
    /// and records an `AuditAction::Write` audit entry.
    pub fn set(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let encrypted = crypto::encrypt(&self.key, value)
            .with_context(|| format!("failed to encrypt secret for key '{key}'"))?;
        let now = chrono::Utc::now().timestamp();

        self.conn
            .execute(
                r#"
                INSERT INTO secrets (project, scope, key, value_blob, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(project, scope, key) DO UPDATE SET
                    value_blob = excluded.value_blob,
                    updated_at = excluded.updated_at;
                "#,
                rusqlite::params![&self.project, &self.scope, key, &encrypted, now, now],
            )
            .with_context(|| format!("failed to persist secret for key '{key}' in database"))?;

        audit::record_audit(&self.conn, &self.project, &self.scope, key, AuditAction::Write)
            .with_context(|| format!("failed to record write audit log for key '{key}'"))?;

        Ok(())
    }

    /// Selects `value_blob` from `secrets` for the current project and scope.
    /// If found, decrypts with `crypto::decrypt` and records `AuditAction::Read`.
    /// If not found, returns `Ok(None)`.
    pub fn get(&self, key: &str) -> anyhow::Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT value_blob FROM secrets WHERE project = ?1 AND scope = ?2 AND key = ?3",
            )
            .context("failed to prepare select secret query")?;

        let mut rows = stmt
            .query(rusqlite::params![&self.project, &self.scope, key])
            .context("failed to execute select secret query")?;

        if let Some(row) = rows.next().context("failed to read secret row")? {
            let value_blob: Vec<u8> = row.get(0).context("failed to read value_blob column")?;
            let decrypted = crypto::decrypt(&self.key, &value_blob)
                .with_context(|| format!("failed to decrypt secret for key '{key}'"))?;

            audit::record_audit(&self.conn, &self.project, &self.scope, key, AuditAction::Read)
                .with_context(|| format!("failed to record read audit log for key '{key}'"))?;

            Ok(Some(decrypted))
        } else {
            Ok(None)
        }
    }

    /// Returns list of keys present in `secrets` table for this project and scope, sorted alphabetically.
    pub fn list(&self) -> anyhow::Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT key FROM secrets WHERE project = ?1 AND scope = ?2 ORDER BY key ASC",
            )
            .context("failed to prepare list secrets query")?;

        let keys = stmt
            .query_map(rusqlite::params![&self.project, &self.scope], |row| {
                row.get(0)
            })
            .context("failed to execute list secrets query")?
            .collect::<Result<Vec<String>, _>>()
            .context("failed to read secret keys")?;

        Ok(keys)
    }

    /// Deletes the secret row from `secrets` and records `AuditAction::Delete`.
    pub fn delete(&self, key: &str) -> anyhow::Result<()> {
        let rows_affected = self
            .conn
            .execute(
                "DELETE FROM secrets WHERE project = ?1 AND scope = ?2 AND key = ?3",
                rusqlite::params![&self.project, &self.scope, key],
            )
            .with_context(|| format!("failed to delete secret for key '{key}'"))?;

        if rows_affected > 0 {
            audit::record_audit(&self.conn, &self.project, &self.scope, key, AuditAction::Delete)
                .with_context(|| format!("failed to record delete audit log for key '{key}'"))?;
        }

        Ok(())
    }

    /// Retrieves and decrypts all secrets for the current project and scope.
    pub fn get_all(&self) -> anyhow::Result<HashMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT key, value_blob FROM secrets WHERE project = ?1 AND scope = ?2 ORDER BY key ASC",
            )
            .context("failed to prepare get_all secrets query")?;

        let rows = stmt
            .query_map(rusqlite::params![&self.project, &self.scope], |row| {
                let key: String = row.get(0)?;
                let value_blob: Vec<u8> = row.get(1)?;
                Ok((key, value_blob))
            })
            .context("failed to execute get_all query")?;

        let mut result = HashMap::new();
        for item in rows {
            let (key, value_blob) = item.context("failed to read row in get_all")?;
            let decrypted = crypto::decrypt(&self.key, &value_blob)
                .with_context(|| format!("failed to decrypt secret for key '{key}'"))?;
            result.insert(key, decrypted);
        }

        Ok(result)
    }

    /// Retrieves audit records for this project, newest first.
    pub fn get_audit_log(&self) -> anyhow::Result<Vec<audit::AuditEntry>> {
        audit::get_audit_log(&self.conn, &self.project)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_client() -> VaultClient {
        let conn = rusqlite::Connection::open_in_memory().expect("open in memory should succeed");
        let key = [0x42u8; 32];
        VaultClient::from_connection(conn, "my-project", "default", key)
            .expect("client creation should succeed")
    }

    #[test]
    fn test_set_and_get() {
        let client = setup_test_client();

        // Getting non-existent key returns None
        let missing = client.get("NONEXISTENT").unwrap();
        assert_eq!(missing, None);

        // Set and get
        client.set("API_KEY", "super_secret_value_123").unwrap();
        let value = client.get("API_KEY").unwrap();
        assert_eq!(value, Some("super_secret_value_123".to_string()));

        // Verify stored value in database is encrypted binary blob, not plaintext
        let raw_blob: Vec<u8> = client
            .connection()
            .query_row(
                "SELECT value_blob FROM secrets WHERE key = 'API_KEY'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_ne!(raw_blob, b"super_secret_value_123");
        assert!(!String::from_utf8_lossy(&raw_blob).contains("super_secret_value_123"));
    }

    #[test]
    fn test_upsert_behavior() {
        let client = setup_test_client();

        client.set("DB_URL", "postgres://localhost/v1").unwrap();
        assert_eq!(
            client.get("DB_URL").unwrap(),
            Some("postgres://localhost/v1".to_string())
        );

        // Update existing key
        client.set("DB_URL", "postgres://localhost/v2").unwrap();
        assert_eq!(
            client.get("DB_URL").unwrap(),
            Some("postgres://localhost/v2".to_string())
        );

        // Verify exactly one row exists in the secrets table
        let count: i64 = client
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM secrets WHERE project = 'my-project' AND key = 'DB_URL'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_list_secrets() {
        let client = setup_test_client();

        assert_eq!(client.list().unwrap(), Vec::<String>::new());

        client.set("ZEBRA", "1").unwrap();
        client.set("APPLE", "2").unwrap();
        client.set("MANGO", "3").unwrap();

        let keys = client.list().unwrap();
        assert_eq!(keys, vec!["APPLE", "MANGO", "ZEBRA"]);
    }

    #[test]
    fn test_delete_secret() {
        let client = setup_test_client();

        client.set("TOKEN", "temp_token").unwrap();
        assert_eq!(client.get("TOKEN").unwrap(), Some("temp_token".to_string()));

        client.delete("TOKEN").unwrap();
        assert_eq!(client.get("TOKEN").unwrap(), None);
        assert!(client.list().unwrap().is_empty());

        // Deleting non-existent key does not fail
        client.delete("NON_EXISTENT_KEY").unwrap();
    }

    #[test]
    fn test_get_all() {
        let client = setup_test_client();

        client.set("HOST", "127.0.0.1").unwrap();
        client.set("PORT", "8080").unwrap();
        client.set("DEBUG", "true").unwrap();

        let all = client.get_all().unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all.get("HOST").map(|s| s.as_str()), Some("127.0.0.1"));
        assert_eq!(all.get("PORT").map(|s| s.as_str()), Some("8080"));
        assert_eq!(all.get("DEBUG").map(|s| s.as_str()), Some("true"));
    }

    #[test]
    fn test_audit_log_generation() {
        let client = setup_test_client();

        // Set produces Write
        client.set("AUDITED_KEY", "value_1").unwrap();

        // Get produces Read
        client.get("AUDITED_KEY").unwrap();

        // Delete produces Delete
        client.delete("AUDITED_KEY").unwrap();

        // Verify audit log entries (ordered newest first)
        let logs = client.get_audit_log().unwrap();
        assert_eq!(logs.len(), 3);

        assert_eq!(logs[0].action, AuditAction::Delete);
        assert_eq!(logs[0].key, "AUDITED_KEY");
        assert_eq!(logs[0].project, "my-project");
        assert_eq!(logs[0].scope, "default");

        assert_eq!(logs[1].action, AuditAction::Read);
        assert_eq!(logs[1].key, "AUDITED_KEY");

        assert_eq!(logs[2].action, AuditAction::Write);
        assert_eq!(logs[2].key, "AUDITED_KEY");
    }

    #[test]
    fn test_scope_isolation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("scope_test.db");
        let project = "scope-project";
        let key = [0x33u8; 32];

        // Ensure database initialized
        let conn1 = db::open_db(&db_path).unwrap();
        let client_default =
            VaultClient::from_connection(conn1, project, "default", key).unwrap();

        client_default
            .set("DATABASE_URL", "postgres://prod/db")
            .unwrap();

        // Second client pointing to same database file but different scope "staging"
        let conn2 = db::open_db(&db_path).unwrap();
        let client_staging =
            VaultClient::from_connection(conn2, project, "staging", key).unwrap();

        client_staging
            .set("DATABASE_URL", "postgres://stage/db")
            .unwrap();

        // Verify each scope retrieves its own secret
        assert_eq!(
            client_default.get("DATABASE_URL").unwrap(),
            Some("postgres://prod/db".to_string())
        );
        assert_eq!(
            client_staging.get("DATABASE_URL").unwrap(),
            Some("postgres://stage/db".to_string())
        );

        // Verify list for each scope
        assert_eq!(client_default.list().unwrap(), vec!["DATABASE_URL"]);
        assert_eq!(client_staging.list().unwrap(), vec!["DATABASE_URL"]);

        // Deleting from default does not affect staging
        client_default.delete("DATABASE_URL").unwrap();
        assert_eq!(client_default.get("DATABASE_URL").unwrap(), None);
        assert_eq!(
            client_staging.get("DATABASE_URL").unwrap(),
            Some("postgres://stage/db".to_string())
        );
    }

    #[test]
    fn test_project_isolation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("proj_test.db");
        let key = [0x55u8; 32];

        let conn1 = db::open_db(&db_path).unwrap();
        let client_a = VaultClient::from_connection(conn1, "project-a", "default", key).unwrap();
        client_a.set("SECRET_A", "val_a").unwrap();

        let conn2 = db::open_db(&db_path).unwrap();
        let client_b = VaultClient::from_connection(conn2, "project-b", "default", key).unwrap();

        // Project B should not see Project A's secret
        assert_eq!(client_b.get("SECRET_A").unwrap(), None);
        assert_eq!(client_b.list().unwrap(), Vec::<String>::new());

        // Project B's audit log should not contain Project A's audit events
        let logs_b = client_b.get_audit_log().unwrap();
        assert!(logs_b.is_empty());

        let logs_a = client_a.get_audit_log().unwrap();
        assert_eq!(logs_a.len(), 1);
        assert_eq!(logs_a[0].project, "project-a");
    }

    #[test]
    fn test_open_file_db_lifecycle() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("vault.db");
        let project = "lifecycle-project";
        let password = "super_secure_master_password";

        // Clean any keychain state for this project
        let _ = delete_key(project);

        // First open: generates salt, derives key, caches in keychain
        {
            let client = VaultClient::open(&db_path, project, "default", password)
                .expect("first VaultClient::open should succeed");
            client.set("MY_SECRET", "persisted_value").unwrap();
        }

        // Second open: uses existing db and cached key
        {
            let client = VaultClient::open(&db_path, project, "default", password)
                .expect("second VaultClient::open should succeed");
            let secret = client.get("MY_SECRET").unwrap();
            assert_eq!(secret, Some("persisted_value".to_string()));
        }

        // Clean up keychain
        let _ = delete_key(project);
    }

    #[test]
    fn test_wrong_key_decryption_error() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        db::init_db(&conn).unwrap();

        let key1 = [0x11u8; 32];
        let key2 = [0x22u8; 32];

        // Store encrypted secret with key1 directly in DB
        let encrypted = crypto::encrypt(&key1, "secret_message").unwrap();
        conn.execute(
            "INSERT INTO secrets (project, scope, key, value_blob, created_at, updated_at) VALUES ('p', 'default', 'K', ?1, 0, 0)",
            rusqlite::params![&encrypted],
        ).unwrap();

        // Client using key2 fails to decrypt
        let client_wrong_key = VaultClient::from_connection(conn, "p", "default", key2).unwrap();
        let result = client_wrong_key.get("K");
        assert!(result.is_err(), "decryption with wrong key must fail");
    }
}
