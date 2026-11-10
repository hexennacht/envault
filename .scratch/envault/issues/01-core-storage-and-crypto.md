# 01: Core Storage and Crypto Foundation

**What to build:** The cryptographic and database storage engine that secures secrets at rest. It provides SQLite schema creation with WAL mode, Argon2id key derivation, AES-256-GCM symmetric encryption/decryption with unique IVs and authentication tags, and audit logging persistence.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Cargo.toml configured with all required dependencies matching SPEC §10.
- [ ] SQLite database connection and migration creating `secrets`, `audit_log`, and `vault_meta` tables in `src/vault/db.rs`.
- [ ] Argon2id key derivation (`derive_key`) and salt generation (`generate_salt`) in `src/vault/kdf.rs`.
- [ ] AES-256-GCM `encrypt` and `decrypt` in `src/vault/crypto.rs` using the payload format `IV (12 bytes) | ciphertext | auth tag (16 bytes)`.
- [ ] Audit log entry insertion and query methods in `src/vault/audit.rs`.
- [ ] Unit tests for crypto round-trip, tampered ciphertext detection, and KDF reproducibility.
