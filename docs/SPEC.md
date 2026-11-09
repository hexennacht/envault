# envault — Technical Specification

**Version:** 0.1.0  
**Status:** Draft  
**Language:** Rust  
**Target Reader:** Junior Rust developer

---

## 1. Purpose

envault is a command-line tool and terminal UI for managing development secrets.
It stores secrets in an encrypted local vault.
It injects approved secrets into child processes as environment variables.
It prevents secret values from appearing in terminal output.

envault combines two roles:

- **Vault** — stores and encrypts secrets (similar to cloakenv).
- **Schema enforcer** — validates secrets against typed rules (similar to varlock).

---

## 2. Scope

This document covers:

- MVP features and their acceptance criteria.
- Application architecture and module layout.
- TUI screens and keyboard navigation.
- CLI commands and their flags.
- Security model and encryption scheme.
- Data models (structs and enums).
- Rust crates to use.
- Build phases and task order.

This document does not cover:

- Cloud sync.
- Team sharing.
- Plugins or external secret providers.

---

## 3. Feature List

### 3.1 MVP Features (Phase 1 and 2)

| ID | Feature | Description |
|----|---------|-------------|
| F01 | Encrypted vault | Store secret key-value pairs with AES-256-GCM encryption |
| F02 | Master password | Derive the encryption key from a password using Argon2id |
| F03 | Local database | Persist the vault in a SQLite file at `~/.config/envault/vault.db` |
| F04 | TUI main screen | Show all keys in a scrollable list |
| F05 | Add secret | Open a form to type a key name and value |
| F06 | View secret | Show the decrypted value in a popup after a confirmation prompt |
| F07 | Delete secret | Delete a key after a confirmation prompt |
| F08 | Search | Filter the key list by typing a search term |
| F09 | Schema file | Read `.env.schema` and validate stored secrets against it |
| F10 | Run command | Inject approved secrets into a child process via `envault run -- <cmd>` |
| F11 | Audit log | Record every read and write with a timestamp |
| F12 | Project detection | Detect the project from `Cargo.toml`, `package.json`, or `go.mod` |
| F13 | Multi-scope | Separate secrets by scope: `default`, `development`, `test` |

### 3.2 Future Features (Phase 3+)

| ID | Feature |
|----|---------|
| F14 | Export to encrypted `.envault` file |
| F15 | Import from encrypted `.envault` file |
| F16 | Log redaction (mask secret values in child process stdout) |
| F17 | Variable expansion (`${VAR}` references in schema) |
| F18 | Generate passphrase |
| F19 | Backup directory auto-sync |

---

## 4. Architecture

### 4.1 Module Layout

```
src/
├── main.rs           # Entry point: parse CLI args, route to TUI or command
├── cli/
│   ├── mod.rs        # CLI argument definitions (clap)
│   ├── run.rs        # `envault run` command implementation
│   └── audit.rs      # `envault audit` command implementation
├── tui/
│   ├── mod.rs        # TUI entry point: initialise terminal, start event loop
│   ├── app.rs        # App state struct and state machine
│   ├── screens/
│   │   ├── secret_list.rs   # Main list screen
│   │   ├── add_secret.rs    # Add/edit form screen
│   │   └── audit_log.rs     # Audit log screen
│   └── components/
│       ├── popup.rs         # Confirmation popup widget
│       ├── search_bar.rs    # Search input component
│       └── status_bar.rs    # Bottom status bar
├── vault/
│   ├── mod.rs        # Public vault API
│   ├── db.rs         # SQLite connection and migrations
│   ├── crypto.rs     # Encrypt and decrypt functions
│   ├── kdf.rs        # Key derivation from password (Argon2id)
│   ├── keychain.rs   # OS keychain storage for derived key
│   └── audit.rs      # Audit log repository
├── schema/
│   ├── mod.rs        # Public schema API
│   ├── parser.rs     # Parse .env.schema files
│   ├── validator.rs  # Validate secret values against schema rules
│   └── types.rs      # Schema type definitions
└── project/
    ├── mod.rs        # Public project API
    └── detector.rs   # Detect project root and name
```

### 4.2 Data Flow

```
User input
    │
    ▼
TUI / CLI
    │
    ▼
Vault API (src/vault/mod.rs)
    │         │
    ▼         ▼
 crypto.rs   db.rs
 (encrypt)  (SQLite)
    │
    ▼
OS keychain (src/vault/keychain.rs)
(stores derived key between sessions)
```

### 4.3 Storage Location

| Item | Path |
|------|------|
| Vault database | `~/.config/envault/vault.db` |
| Config file | `~/.config/envault/config.toml` |
| Audit log table | Inside `vault.db` |

---

## 5. Security Model

### 5.1 Encryption

envault uses symmetric encryption with a key it derives from the master password.

| Step | Algorithm | Detail |
|------|-----------|--------|
| Key derivation | Argon2id | m=65536, t=3, p=4. Salt stored in `vault.db`. |
| Encryption at rest | AES-256-GCM | Unique 12-byte IV per record |
| Auth tag | 16 bytes | Included in the stored ciphertext payload |
| Key storage | OS keychain | Derived key cached for one session |

### 5.2 Stored Record Format

Each secret value in the database stores a binary payload with this layout:

```
IV (12 bytes) | ciphertext (N bytes) | auth tag (16 bytes)
```

The database stores this payload as a BLOB.

### 5.3 Master Password

- The user sets a master password on first run.
- envault derives a 32-byte key from the password using Argon2id.
- envault stores the derived key in the OS keychain after the first successful unlock.
- Subsequent operations in the same session use the cached key.
- The user does not need to type the password again until a new session starts.

### 5.4 What envault Does Not Store

- envault never stores the master password.
- envault never stores a plaintext secret value outside the encrypted payload.

---

## 6. Data Models

### 6.1 Database Schema

```sql
CREATE TABLE secrets (
    id          INTEGER PRIMARY KEY,
    project     TEXT NOT NULL,
    scope       TEXT NOT NULL DEFAULT 'default',
    key         TEXT NOT NULL,
    value_blob  BLOB NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    UNIQUE(project, scope, key)
);

CREATE TABLE audit_log (
    id          INTEGER PRIMARY KEY,
    project     TEXT NOT NULL,
    scope       TEXT NOT NULL,
    key         TEXT NOT NULL,
    action      TEXT NOT NULL,
    at          INTEGER NOT NULL
);

CREATE TABLE vault_meta (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    salt        BLOB NOT NULL,
    created_at  INTEGER NOT NULL
);
```

### 6.2 Rust Structs

```rust
// src/vault/mod.rs
pub struct Secret {
    pub id: i64,
    pub project: String,
    pub scope: String,
    pub key: String,
    pub value_blob: Vec<u8>,   // encrypted payload
    pub created_at: i64,
    pub updated_at: i64,
}

// src/vault/crypto.rs
pub struct EncryptedPayload {
    pub iv: [u8; 12],
    pub ciphertext: Vec<u8>,
    pub tag: [u8; 16],
}

// src/vault/audit.rs
pub struct AuditEntry {
    pub id: i64,
    pub project: String,
    pub scope: String,
    pub key: String,
    pub action: AuditAction,
    pub at: i64,
}

pub enum AuditAction {
    Read,
    Write,
    Delete,
}

// src/schema/types.rs
pub struct SchemaEntry {
    pub key: String,
    pub required: bool,
    pub sensitive: bool,
    pub description: Option<String>,
    pub schema_type: SchemaType,
    pub default_value: Option<String>,
}

pub enum SchemaType {
    String,
    Port,
    Url,
    Bool,
    Integer,
    Enum(Vec<String>),
}
```

### 6.3 App State — Design Graph

The TUI event loop follows the graph below.
Each node carries three channels: A (success data), E (failure modes), R (dependencies).

```text
graph: [proposed] envault TUI event loop

Production:
main [new]
  → App::new [new]
    → ProjectDetector::detect [new]
    → VaultClient::list [new]
    → SchemaParser::parse [new]
  → tui::run [new]
    → terminal::draw [new]
      → App::render [new]
        → [if screen == SecretList] render_secret_list [new]
        → [if screen == AddSecret] render_add_form [new]
        → [if screen == AuditLog] render_audit_log [new]
        → [if popup.is_some()] render_popup [new]
    → event::poll [crossterm]
    → event::read [crossterm]
    → App::handle_event [new]
      → [if popup.is_some()] App::handle_popup_event [new]
      → [if screen == SecretList] App::handle_list_event [new]
      → [if screen == AddSecret] App::handle_form_event [new]
      → [if screen == AuditLog] App::handle_audit_event [new]

E:
  VaultClient::get      DecryptFail, KeyNotFound   escape → PopupKind::Error
  VaultClient::set      EncryptFail, DbWriteFail   escape → PopupKind::Error
  VaultClient::list     DbReadFail                 escape → PopupKind::Error, keep old list
  SchemaParser::parse   FileNotFound, ParseError   escape → empty schema, non-fatal
  terminal::draw        IoError                    die (terminal is broken)
  event::read           IoError                    die (stdin is broken)

R:
  App::new              VaultClient, ProjectDetector, SchemaParser
  App::handle_*_event   VaultClient (for get, set, delete, list)
  tui::run              DefaultTerminal (from ratatui::init)
```

**Event dispatch order:**

1. If `popup` is `Some`, route the key event to `handle_popup_event`. No other handler runs.
2. If `popup` is `None`, route the key event to the handler for the current `screen`.
3. Each screen handler has one `match key.code` block. This is the seam.

**Design rules:**

- The happy path body is A. Each handler method reads top to bottom as a list of state transitions.
- The seam is E. Vault operation errors become `PopupKind::Error` and never propagate past the handler.
- Every variant of `PopupKind` carries all data it needs. No separate `message` field.
- `FocusZone` replaces the old `search_active: bool` flag to prevent invariant desync.

```rust
// src/tui/app.rs

/// Top-level application state. Owns all data the TUI reads and writes.
pub struct App {
    pub mode: AppMode,
    pub screen: Screen,
    pub secrets: Vec<SecretMeta>,  // key names only, no plaintext values
    pub selected: usize,
    pub search_query: String,
    pub focus: FocusZone,
    pub project: String,
    pub scope: String,
    pub schema: Vec<SchemaEntry>,
    pub status: StatusBar,
    pub popup: Option<PopupKind>,
}

/// SecretMeta is one row in the secret list.
/// It never holds a plaintext value.
pub struct SecretMeta {
    pub key: String,
    pub required: bool,
    pub sensitive: bool,
    pub has_value: bool,
}

pub enum AppMode {
    Running,
    Quit,
}

/// Screen determines which view the TUI renders.
/// Each variant carries the state that only that screen needs.
pub enum Screen {
    SecretList,
    AddSecret(AddSecretForm),
    EditSecret(EditSecretForm),
    AuditLog(AuditLogState),
}

/// FocusZone tracks which part of the SecretList screen accepts input.
/// This replaces a separate `search_active: bool` field.
/// One enum = one source of truth.
pub enum FocusZone {
    List,
    SearchBar,
}

pub struct AddSecretForm {
    pub key_input: String,
    pub value_input: String,
    pub focused_field: FormField,
    pub cursor_pos: usize,
}

pub struct EditSecretForm {
    pub key: String,           // read-only, shows which secret the user edits
    pub value_input: String,
    pub cursor_pos: usize,
}

pub struct AuditLogState {
    pub entries: Vec<AuditEntry>,
    pub scroll_offset: usize,
    pub selected: usize,
}

pub enum FormField {
    Key,
    Value,
}

/// PopupKind is an overlay on top of any screen.
/// Each variant carries all data it needs. No separate `message` field.
pub enum PopupKind {
    ConfirmDelete { key: String },
    ShowValue { key: String, value: String },
    Error { title: String, detail: String },
}

pub struct StatusBar {
    pub text: String,
    pub level: StatusLevel,
}

pub enum StatusLevel {
    Info,
    Error,
}
```

---

## 7. The `.env.schema` File Format

envault reads an optional `.env.schema` file at the project root.
The schema format is compatible with the `@env-spec` standard from varlock.

### 7.1 Syntax Rules

- Lines that start with `#` are comments.
- A comment that starts with `# @` contains a decorator.
- A line in the form `KEY=` or `KEY=value` defines a schema entry.
- Comments that appear directly before a `KEY=` line attach to that entry.

### 7.2 Supported Decorators

| Decorator | Type | Description |
|-----------|------|-------------|
| `@required` | boolean | The secret must have a value at run time |
| `@sensitive` | boolean | The secret is a secret and envault must redact it in output |
| `@type=string` | string | The value must be a non-empty string |
| `@type=port` | port | The value must be an integer between 1 and 65535 |
| `@type=url` | url | The value must start with `http://` or `https://` |
| `@type=bool` | bool | The value must be `true` or `false` |
| `@type=integer` | integer | The value must be a whole number |
| `@type=enum(a,b,c)` | enum | The value must match one of the listed options |

### 7.3 Example File

```dotenv
# Application environment flag
# @required @type=enum(development,test,production)
APP_ENV=development

# TCP port for the API server
# @required @type=port
API_PORT=8080

# Stripe secret key
# @required @sensitive @type=string
STRIPE_SECRET_KEY=
```

### 7.4 Parsing Rules

The parser reads the file line by line. It follows these rules:

1. Skip blank lines.
2. If the line starts with `# @`, collect decorator annotations.
3. If the line starts with `#` without `@`, collect as a description comment.
4. If the line contains `=`, create a `SchemaEntry` with any collected annotations.
5. Reset the collected annotations after each entry.

---

## 8. TUI Screens

### 8.1 Screen Transition Graph

The TUI has four screens and one overlay layer.
The diagram below shows all valid screen transitions.

```text
               ┌───────────────────────────────┐
               │          SecretList           │◀──────────────┐
               │    (focus: List | Search)     │               │
               └─┬─────────┬─────────┬───────┬─┘               │
                 │         │         │       │                 │
            (a)  │    (e)  │    (A)  │   (d) │                 │
                 ▼         ▼         │       │                 │
          ┌──────────┐ ┌──────────┐  │       ▼                 │
          │AddSecret │ │EditSecret│  │   ┌──────────────────┐  │
          └────┬─────┘ └────┬─────┘  │   │ConfirmDelete     │──┘
               │            │        │   │(popup overlay)   │
               │ (submit/   │ (submit│   └──────────────────┘
               │  cancel)   │ /cancel│                         │
               ▼            ▼        ▼                         │
          ┌────────────────────────────┐                       │
          │         SecretList         │                       │
          └────────────────────────────┘                       │
                                     │                         │
                                     │ (A)                     │
                                     ▼                         │
                               ┌──────────┐                    │
                               │ AuditLog │────────────────────┘
                               └──────────┘    (Esc or q)
```

**Popup Layer:**

Popups render on top of whichever screen is active.
When a popup is open, all keystrokes go to the popup handler first.
Closing the popup restores focus to the underlying screen.

```
Any Screen ──(error occurs)──▶ PopupKind::Error
SecretList ──(Enter)──▶ PopupKind::ShowValue
SecretList ──(d)──▶ PopupKind::ConfirmDelete
```

---

### 8.2 SecretList Screen

The main screen shows all secrets for the active project and scope.
It has two focus zones: `List` and `SearchBar`.

```
┌─ envault ─── project: my-app ── scope: development ─────────────────────┐
│ [/] Search: _                                                            │
├──────────────────────────────────────────────────────────────────────────┤
│ ▶ DATABASE_URL                        ● set       required  sensitive    │
│   STRIPE_SECRET_KEY                   ○ missing   required  sensitive    │
│   APP_ENV                             ● set       required               │
│   API_PORT                            ● set       required               │
├──────────────────────────────────────────────────────────────────────────┤
│ a=add  e=edit  d=delete  Enter=view  A=audit  /=search  q=quit          │
└──────────────────────────────────────────────────────────────────────────┘
```

**Indicator meanings:**

- `● set` — the secret exists in the vault.
- `○ missing` — the `.env.schema` file lists this key, but the vault has no value for it.

**Keyboard bindings (when `focus == FocusZone::List`):**

| Key | Action |
|-----|--------|
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `a` | Open `AddSecret` screen |
| `e` | Open `EditSecret` screen for the selected secret |
| `Enter` | Read secret from vault and show in `ShowValue` popup |
| `d` | Open `ConfirmDelete` popup for the selected secret |
| `A` | Open `AuditLog` screen |
| `/` | Switch focus to `SearchBar` |
| `q` | Quit the application |

**Keyboard bindings (when `focus == FocusZone::SearchBar`):**

| Key | Action |
|-----|--------|
| Any char | Append char to `search_query` and filter list immediately |
| `Backspace` | Delete last char from `search_query` |
| `Esc` | Clear `search_query` and switch focus back to `List` |
| `Enter` | Keep `search_query` and switch focus back to `List` |

---

### 8.3 AddSecret Screen

Use this screen to create a new secret.
It has two input fields: Key and Value.

```
┌─ Add Secret ─────────────────────────────────────────────────────────────┐
│                                                                           │
│  Key:   DATABASE_URL_                                                     │
│                                                                           │
│  Value: ████████████████████_                                             │
│                                                                           │
│  [Tab] switch field   [Enter] save   [Esc] cancel                        │
└───────────────────────────────────────────────────────────────────────────┘
```

**Rules:**

- The Value field masks typed input with `█` characters.
- Press `Tab` to switch focus between the Key field and Value field.
- Press `Enter` to submit the form.
  - On submit: call `VaultClient::set`.
  - If `set` succeeds: transition to `SecretList` and reload the list.
  - If `set` fails: show `PopupKind::Error`.
- Press `Esc` to cancel and return to `SecretList` without saving.
- The Key field rejects any character outside `[A-Za-z0-9_]`.
- Submit does not proceed if the Key field is empty.

---

### 8.4 EditSecret Screen

Use this screen to update the value of an existing secret.
The key name stays locked and the user cannot change it.

```
┌─ Edit Secret: DATABASE_URL ──────────────────────────────────────────────┐
│                                                                           │
│  Key:   DATABASE_URL (read-only)                                          │
│                                                                           │
│  New Value: ████████████████████_                                         │
│                                                                           │
│  [Enter] save   [Esc] cancel                                             │
└───────────────────────────────────────────────────────────────────────────┘
```

**Rules:**

- The key name cannot be edited. It shows as read-only text.
- The New Value field starts empty and masks input with `█` characters.
- Press `Enter` to encrypt and store the new value.
- Press `Esc` to cancel and return to `SecretList`.

---

### 8.5 ConfirmDelete Popup

A centered modal overlay that asks the user before deleting a secret.

```
┌─ Confirm Delete ─────────────────────────────────────────────────────────┐
│                                                                           │
│  Delete secret: DATABASE_URL?                                             │
│  This action cannot be undone.                                            │
│                                                                           │
│         [ Yes (Enter) ]     [ No (Esc) ]                                 │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

**Rules:**

- Press `Enter`: call `VaultClient::delete(key)`.
  - On success: close popup, reload secret list, update status bar.
  - On failure: close this popup and open `PopupKind::Error`.
- Press `Esc`: close popup without deleting.

---

### 8.6 ShowValue Popup

A centered modal overlay showing the decrypted value of a secret.

```
┌─ DATABASE_URL ───────────────────────────────────────────────────────────┐
│                                                                           │
│  postgres://user:password@localhost:5432/mydb                             │
│                                                                           │
│  [c] copy to clipboard   [Esc] close                                      │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

**Rules:**

- The popup shows the decrypted plaintext value.
- Press `c`: copy value to OS clipboard and update status bar with `"Copied to clipboard"`.
- Press `Esc` or `Enter`: close popup and wipe the decrypted value from memory.

---

### 8.7 Error Popup

A centered modal overlay showing an error that occurred during a vault operation.
This is the seam where the E channel surfaces to the user.

```
┌─ Error: Decryption Failed ───────────────────────────────────────────────┐
│                                                                           │
│  Could not decrypt secret "DATABASE_URL".                                │
│  Detail: Invalid auth tag. Ciphertext may be corrupted.                 │
│                                                                           │
│  [Esc] dismiss                                                            │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

**Rules:**

- Renders a bordered `Block` with title `Error: <title>` in red text.
- Press `Esc` or `Enter`: dismiss the popup and return to the underlying screen.
- Errors never crash the application. They always surface as this popup.

---

### 8.8 AuditLog Screen

A full-screen view of the audit log table.
Supports scrolling through past operations.

```
┌─ Audit Log ─── project: my-app ──────────────────────────────────────────┐
│ Time                  Scope         Action   Key                          │
│ 2025-01-15 10:32:01   development   READ     DATABASE_URL                 │
│ 2025-01-15 10:30:45   development   WRITE    STRIPE_SECRET_KEY            │
│ 2025-01-15 10:30:44   development   WRITE    DATABASE_URL                 │
│ 2025-01-15 09:12:00   default       DELETE   OLD_API_KEY                  │
├──────────────────────────────────────────────────────────────────────────┤
│ j/k=scroll  g=top  G=bottom  Esc/q=back                                  │
└──────────────────────────────────────────────────────────────────────────┘
```

**Keyboard bindings:**

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down one row |
| `k` / `↑` | Scroll up one row |
| `g` | Jump to top of log |
| `G` | Jump to bottom of log |
| `Esc` / `q` | Return to `SecretList` screen |

---

## 9. CLI Commands

### 9.1 Command Overview

| Command | Description |
|---------|-------------|
| `envault` | Open the TUI |
| `envault run -- <cmd>` | Run a command with secrets injected as env vars |
| `envault set KEY=VALUE` | Set a secret from the command line |
| `envault get KEY` | Print the decrypted value of a key to stdout |
| `envault list` | List all key names for the current project |
| `envault audit` | Print the audit log to stdout |
| `envault validate` | Validate stored secrets against `.env.schema` |

### 9.2 Global Flags

| Flag | Description |
|------|-------------|
| `--project <name>` | Override the detected project name |
| `--scope <name>` | Use a specific scope (default: `default`) |
| `--db <path>` | Use a custom vault database path |

### 9.3 `envault run`

**Usage:**
```
envault run [--scope <scope>] -- <command> [args...]
```

**What it does:**

1. Detect the project.
2. Load all secrets for the project and scope from the vault.
3. Decrypt each secret value.
4. Merge the decrypted values into the current process environment.
5. Spawn the given command with the merged environment.
6. Wait for the command to exit.
7. Exit with the same exit code as the command.

**Example:**
```bash
envault run -- cargo run
envault run --scope test -- cargo test
```

### 9.4 `envault validate`

**What it does:**

1. Load `.env.schema` from the current directory.
2. Load all secrets for the current project and scope.
3. For each schema entry that has `@required`, test that a secret value exists.
4. For each secret value that exists, test that the value matches the `@type`.
5. Print a table of results.
6. Exit with code 1 if any required secret is missing or any value fails type testing.

**Example output:**
```
Validating project: my-app (scope: development)

  KEY                  STATUS    DETAIL
  DATABASE_URL         ✓ OK
  STRIPE_SECRET_KEY    ✗ MISSING  required
  APP_ENV              ✓ OK
  API_PORT             ✗ INVALID  expected port (1-65535), got "not_a_port"

2 errors found.
```

---

## 10. Rust Crates

### 10.1 Required Crates

| Crate | Version | Purpose |
|-------|---------|---------|
| `ratatui` | `0.29` | TUI rendering |
| `crossterm` | `0.28` | Terminal backend for ratatui |
| `clap` | `4` | CLI argument parsing |
| `rusqlite` | `0.32` | SQLite database access |
| `aes-gcm` | `0.10` | AES-256-GCM encryption |
| `argon2` | `0.5` | Argon2id key derivation |
| `rand` | `0.8` | Secure random IV generation |
| `keyring` | `3` | OS keychain access |
| `anyhow` | `1` | Error handling |
| `serde` | `1` | Serialization for config |
| `toml` | `0.8` | Config file parsing |

### 10.2 Optional Crates (Phase 2+)

| Crate | Purpose |
|-------|---------|
| `color-eyre` | Richer error output in TUI |
| `chrono` | Timestamp formatting in audit log |
| `unicode-width` | Accurate cursor position for wide characters |

### 10.3 Cargo.toml (Starting Point)

```toml
[package]
name = "envault"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "envault"
path = "src/main.rs"

[dependencies]
ratatui = "0.29"
crossterm = "0.28"
clap = { version = "4", features = ["derive"] }
rusqlite = { version = "0.32", features = ["bundled"] }
aes-gcm = "0.10"
argon2 = "0.5"
rand = "0.8"
keyring = "3"
anyhow = "1"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
chrono = "0.4"
zeroize = "1"
```

---

## 11. Build Plan

The build plan has three phases.
Complete each phase in order.
Run the acceptance tests before moving to the next phase.

### Phase 1: Core Vault (No TUI)

**Goal:** Store and retrieve encrypted secrets from the command line.

**Tasks:**

1. Set up the `Cargo.toml` with all required dependencies.
2. Create `src/vault/db.rs`:
   - Open the SQLite database.
   - Run migrations to create the three tables.
3. Create `src/vault/kdf.rs`:
   - Implement `derive_key(password: &str, salt: &[u8]) -> [u8; 32]` using Argon2id.
   - Implement `generate_salt() -> [u8; 32]` using `rand::RngCore`.
4. Create `src/vault/crypto.rs`:
   - Implement `encrypt(key: &[u8; 32], plaintext: &str) -> Vec<u8>`.
   - Implement `decrypt(key: &[u8; 32], payload: &[u8]) -> anyhow::Result<String>`.
   - The encrypted payload format is: `IV (12 bytes) | ciphertext | auth tag (16 bytes)`.
5. Create `src/vault/keychain.rs`:
   - Implement `store_key(project: &str, key: &[u8; 32]) -> anyhow::Result<()>`.
   - Implement `load_key(project: &str) -> anyhow::Result<Option<[u8; 32]>>`.
6. Create `src/vault/mod.rs`:
   - Implement `VaultClient` struct.
   - Implement `set(key, value) -> anyhow::Result<()>`.
   - Implement `get(key) -> anyhow::Result<Option<String>>`.
   - Implement `list() -> anyhow::Result<Vec<SecretMeta>>`.
   - Implement `delete(key) -> anyhow::Result<()>`.
7. Create `src/cli/mod.rs` with clap structs.
8. Create `src/main.rs` to route `set`, `get`, `list` commands to the vault.

**Phase 1 Acceptance Test:**

```bash
# These commands must work without errors.
envault set MY_KEY=hello_world
envault get MY_KEY          # prints: hello_world
envault list                # prints: MY_KEY
```

---

### Phase 2: TUI

**Goal:** Add the ratatui terminal interface on top of the vault.

**Tasks:**

1. Create `src/tui/app.rs` with the `App` struct and all state enums.
2. Create `src/tui/mod.rs`:
   - Initialize the crossterm terminal.
   - Restore the terminal on exit.
   - Run the event loop: `poll`, `read`, `draw`.
3. Create `src/tui/screens/secret_list.rs`:
   - Draw the outer `Block` with the project and scope in the title.
   - Draw the secret list using the `List` widget.
   - Highlight the selected item.
   - Show schema metadata (required, sensitive) next to each key name.
4. Create `src/tui/components/search_bar.rs`:
   - Render a `Paragraph` input at the top of the list.
   - Filter the list when search is active.
5. Create `src/tui/components/popup.rs`:
   - Render a centered `Block` with `Clear` underneath it.
   - Support `Confirm`, `ShowValue`, and `Error` popup kinds.
6. Create `src/tui/screens/add_secret.rs`:
   - Render a two-field form.
   - Mask the value field with `█` characters.
   - Handle `Tab` key to switch focus.
7. Create `src/tui/screens/audit_log.rs`:
   - Render a scrollable `Table` with time, action, and key columns.
8. Update `src/main.rs`:
   - Start the TUI when the user runs `envault` with no subcommand.

**Phase 2 Acceptance Test:**

```bash
# Running `envault` opens the TUI.
# The secret list shows all stored keys.
# The user can add, view, and delete secrets.
# The user can search by typing after pressing '/'.
# Pressing 'q' exits cleanly.
```

---

### Phase 3: Schema and Run

**Goal:** Read `.env.schema` and inject secrets into child processes.

**Tasks:**

1. Create `src/schema/parser.rs`:
   - Implement `parse_schema_file(path: &Path) -> anyhow::Result<Vec<SchemaEntry>>`.
   - Follow the parsing rules in Section 7.4.
2. Create `src/schema/validator.rs`:
   - Implement `validate(entries: &[SchemaEntry], secrets: &HashMap<String, String>) -> Vec<ValidationError>`.
3. Create `src/project/detector.rs`:
   - Walk up the directory tree.
   - Return the first directory that contains `Cargo.toml`, `package.json`, `go.mod`, or `.envault`.
   - Use the directory name as the project name.
4. Create `src/cli/run.rs`:
   - Implement the `envault run` command.
   - Follow the steps in Section 9.3.
5. Create `src/cli/audit.rs`:
   - Implement the `envault audit` command.
   - Print the audit log in a table format.
6. Add `envault validate` to the CLI.

**Phase 3 Acceptance Test:**

```bash
# .env.schema contains DATABASE_URL with @required.
# The vault has DATABASE_URL set.
envault validate               # exits 0, prints "OK"

# Remove DATABASE_URL from vault.
envault validate               # exits 1, prints "MISSING" for DATABASE_URL

# Run a command with secrets injected.
envault run -- env | grep DATABASE_URL   # prints the decrypted value
```

---

## 12. Error Handling Rules

Follow these rules in all modules:

1. Use `anyhow::Result` as the return type for all functions that can fail.
2. Use `anyhow::Context` to add a description to each error before returning it.
3. Never call `unwrap()` or `expect()` in production code paths.
4. The TUI must catch all errors from vault operations and show them in an error popup.
5. The error popup must let the user press `Esc` to dismiss and continue.

---

## 13. Testing Plan

### 13.1 Unit Tests

Write a unit test for each of these functions:

| Module | Function | Test cases |
|--------|----------|------------|
| `vault/crypto.rs` | `encrypt` and `decrypt` | Roundtrip: encrypt then decrypt returns the original string |
| `vault/crypto.rs` | `decrypt` | Returns error when an attacker changes the payload |
| `vault/kdf.rs` | `derive_key` | Same password and salt produce the same key |
| `vault/kdf.rs` | `generate_salt` | Two calls produce different values |
| `schema/parser.rs` | `parse_schema_file` | Parses a file with all decorator types correctly |
| `schema/validator.rs` | `validate` | Returns an error for a missing required key |
| `schema/validator.rs` | `validate` | Returns an error for a port value that is out of range |
| `project/detector.rs` | `detect` | Returns the directory that contains `Cargo.toml` |

### 13.2 Integration Tests

Write an integration test for each CLI command:

| Command | Test |
|---------|------|
| `envault set` | Set a key, then `get` it and test that the value matches |
| `envault run` | Run `env` as the child command and test that the key appears in its output |
| `envault validate` | Test exit code 0 when all required keys are present |
| `envault validate` | Test exit code 1 when a required key is missing |

### 13.3 Test Database

Use a temporary SQLite database (`:memory:` or `tempfile`) for all tests.
Tests must not read from or write to `~/.config/envault/vault.db`.

---

## 14. Acceptance Criteria Summary

| Criterion | Pass condition |
|-----------|----------------|
| Vault roundtrip | Encrypt and decrypt produces the original plaintext |
| No plaintext on disk | `strings vault.db` contains no secret value |
| TUI starts | `envault` opens without errors on macOS, Linux, and Windows |
| Run command | `envault run -- env` shows injected secrets |
| Schema validation | Missing required secret causes exit code 1 |
| Audit log | Every `get` and `set` operation appears in the audit log |
| Search | Typing in `/` mode filters the shown list |
| Quit | Pressing `q` restores the terminal and exits with code 0 |

---

## Appendix A: Key Design Decisions

### A.1 Why SQLite?

SQLite is a single-file database that ships with Rust via the `rusqlite` crate with the `bundled` feature.
It supports WAL mode for crash safety.
It does not require a server process.

### A.2 Why Argon2id?

Argon2id is the current recommended algorithm for password-based key derivation (OWASP, 2024).
It resists GPU and ASIC attacks with its memory-hard design.
The `argon2` crate in Rust implements it without C dependencies.

### A.3 Why Not ECIES?

ECIES (as used by varlock) requires asymmetric key management.
For a single-user local vault, symmetric key derivation from a password is simpler and equally secure.
The spec may add ECIES in a future phase for key-sharing features.

### A.4 Why AES-256-GCM?

AES-256-GCM provides both confidentiality and integrity in one operation.
The `aes-gcm` crate implements it in pure Rust with no OpenSSL dependency.
The auth tag detects any tampering with the ciphertext.

---

## Appendix B: Glossary

| Term | Definition |
|------|-----------|
| Secret | A key-value pair stored in the encrypted vault |
| Key | The name of a secret, for example `DATABASE_URL` |
| Value | The plaintext content of a secret |
| Scope | A namespace for secrets within a project, for example `development` or `test` |
| Schema | A `.env.schema` file that describes the expected keys and their types |
| Master password | The password the user types to unlock the vault |
| Derived key | The 32-byte AES key produced by running Argon2id on the master password |
| IV | Initialization vector — 12 random bytes that make each encryption unique |
| Auth tag | 16 bytes that AES-256-GCM adds to detect any change to the ciphertext |
| Payload | The binary blob stored in the database: `IV + ciphertext + auth tag` |
| Project | The detected name of the current code repository |
| Audit log | A record of every read and write operation on the vault |
