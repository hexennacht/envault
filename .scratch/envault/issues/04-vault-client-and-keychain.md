# 04: Vault Client and Keychain

**What to build:** High-level `VaultClient` API that orchestrates database persistence, key derivation, OS keychain key caching, AES encryption, and audit log generation for secret CRUD operations.

**Blocked by:** 01: Core Storage and Crypto Foundation

**Status:** completed

- [x] `keychain.rs` provides `store_key` and `load_key` with safe fallback when headless/keyring is unavailable.
- [x] `VaultClient` implemented in `src/vault/mod.rs` supporting `set`, `get`, `list`, `delete`, and `get_all` for a given project and scope.
- [x] Master password prompting and salt initialization on first run.
- [x] Automatic audit logging for `get` (Read), `set` (Write), and `delete` (Delete) actions.
- [x] Integration tests verifying secret lifecycle on temporary databases without exposing plaintext on disk.
