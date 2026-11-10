# 06: CLI Core Commands

**What to build:** Core command-line interface for managing secrets without the TUI, supporting `set`, `get`, `list`, and `delete` subcommands along with `--project`, `--scope`, and `--db` global flags.

**Blocked by:** 03: Project Detector, 04: Vault Client and Keychain

**Status:** ready-for-agent

- [ ] Clap CLI parser in `src/cli/mod.rs` with `set`, `get`, `list`, `delete` subcommands and global flags (`--project`, `--scope`, `--db`).
- [ ] `envault set KEY=VALUE` stores encrypted secret.
- [ ] `envault get KEY` outputs decrypted plaintext secret.
- [ ] `envault list` outputs list of stored secret keys.
- [ ] `envault delete KEY` deletes the secret.
- [ ] Integration tests testing CLI invocations for round-trip secret operations.
