pub mod audit;
pub mod crypto;
pub mod db;
pub mod kdf;

// Re-exports of key structs and functions
pub use audit::{get_audit_log, record_audit, record_audit_at, AuditAction, AuditEntry};
pub use crypto::{decrypt, decrypt_payload, encrypt, encrypt_payload, EncryptedPayload};
pub use db::{
    default_db_path, get_vault_salt, init_db, open_db, open_in_memory, set_vault_salt, Secret,
};
pub use kdf::{derive_key, generate_salt};
