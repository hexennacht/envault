# envault

`envault` manages development secrets in an encrypted local database.
It enforces typed validation rules through schema files.
It injects decrypted values into child processes without writing plaintext secrets to disk.

## Features

- **Encrypted storage**: Encrypts secret values with AES-256-GCM.
- **Key derivation**: Derives encryption keys using Argon2id with memory-hard parameters.
- **OS keychain integration**: Caches session keys in the system keychain for quick access.
- **Schema enforcement**: Validates secret keys and types against `.env.schema` files.
- **Process injection**: Spawns child commands with secrets injected into the process environment.
- **Terminal UI**: Provides an interactive terminal interface built with Ratatui.
- **Project and scope isolation**: Separates secrets by project name and environment scope.
- **Audit logging**: Records secret creation, read access, and deletion events with timestamps.

## Installation

### Prerequisites

Install the Rust toolchain (Rust 2024 edition or newer):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build from source

Clone the repository and install the binary:

```bash
git clone https://github.com/hexennacht/envault.git
cd envault
cargo install --path .
```

To build a release binary without installing:

```bash
cargo build --release
```

The build places the executable at `target/release/envault`.

## Quick Start

1. Store a secret:

```bash
envault set DATABASE_URL="postgres://localhost:5432/dev"
```

On first use, `envault` prompts you to choose a master password.

2. Read the secret value:

```bash
envault get DATABASE_URL
```

3. Run an application with decrypted secrets:

```bash
envault run -- npm start
```

4. Open the terminal user interface:

```bash
envault
```

## CLI Reference

### Commands

| Command | Description |
|---|---|
| `envault` | Start the terminal user interface |
| `envault set <KEY=VALUE>` | Store an encrypted secret |
| `envault get <KEY>` | Decrypt and print a secret value |
| `envault list` | List all stored keys for the current scope |
| `envault delete <KEY>` | Delete a secret (alias: `rm`) |
| `envault run -- <CMD>` | Run a command with injected secrets |
| `envault validate` | Validate secrets against `.env.schema` |
| `envault audit` | Print the project audit log |

### Global Options

| Option | Description |
|---|---|
| `--project <NAME>` | Override detected project name |
| `--scope <NAME>` | Target a specific environment scope |
| `--db <PATH>` | Set custom path for the SQLite database |

### Examples

Run tests with test scope secrets:

```bash
envault --scope test run -- cargo test
```

Validate secrets against a custom schema path:

```bash
envault validate --schema ./config/.env.schema
```

Delete a secret key:

```bash
envault delete STRIPE_SECRET_KEY
```

## Schema Enforcement

`envault` reads `.env.schema` files from the project root.
The format uses standard shell assignment syntax with comment decorators.

### Supported Decorators

| Decorator | Type | Description |
|---|---|---|
| `@required` | Flag | Fails validation if the secret is missing |
| `@sensitive` | Flag | Marks the value for output masking |
| `@type=string` | String | Value must be a non-empty string |
| `@type=port` | Integer | Value must be an integer from 1 to 65535 |
| `@type=url` | URL | Value must start with `http://` or `https://` |
| `@type=bool` | Boolean | Value must be `true` or `false` |
| `@type=integer` | Integer | Value must be a valid integer |
| `@type=enum(...)` | Choice | Value must match a comma-separated choice |

### Schema Example

Create `.env.schema` in your project root:

```dotenv
# Environment mode
# @required @type=enum(development,test,production)
APP_ENV=development

# Application port number
# @required @type=port
PORT=8080

# API endpoint URL
# @required @type=url
API_BASE_URL=https://api.example.com

# Stripe private key
# @required @sensitive @type=string
STRIPE_SECRET_KEY=
```

Run validation to validate stored secrets:

```bash
envault validate
```

## Terminal User Interface (TUI)

Start the interface by running `envault` without arguments:

```bash
envault
```

### Key Bindings

#### Main Secret List

| Key | Action |
|---|---|
| `j` / `Down` | Move cursor down |
| `k` / `Up` | Move cursor up |
| `a` | Open add secret form |
| `e` | Open edit form for selected secret |
| `Enter` | Display decrypted value popup |
| `d` | Open delete confirmation popup |
| `A` | Open audit log viewer |
| `/` | Focus search bar |
| `q` | Exit application |

#### Search Bar

| Key | Action |
|---|---|
| `Type characters` | Filter secrets list in real time |
| `Backspace` | Delete previous character |
| `Enter` | Keep current filter and return to list |
| `Esc` | Clear filter and return to list |

#### Form Inputs (Add and Edit)

| Key | Action |
|---|---|
| `Tab` | Switch between key and value fields |
| `Enter` | Save secret to vault |
| `Esc` | Cancel and return to secret list |

#### Audit Log Viewer

| Key | Action |
|---|---|
| `j` / `Down` | Scroll down one row |
| `k` / `Up` | Scroll up one row |
| `g` | Jump to start of log |
| `G` | Jump to end of log |
| `Esc` / `q` | Return to secret list |

## Configuration

You can configure persistent defaults in `~/.config/envault/config.toml`:

```toml
# Default environment scope when --scope is omitted
default_scope = "development"

# Default SQLite database path override
db = "~/.config/envault/vault.db"
```

## Security Architecture

`envault` applies defense-in-depth principles:

- **Argon2id Key Derivation**: Derives a 256-bit key using $m=65536$ KiB memory, $t=3$ iterations, and $p=4$ parallelism threads.
- **AES-256-GCM Encryption**: Generates a unique 12-byte initialization vector (IV) for each stored secret value.
- **Authentication Tags**: Appends a 16-byte authentication tag to detect any ciphertext tampering.
- **Keyring Storage**: Stores the derived key in the OS keychain for the active session.
- **Memory Zeroization**: Clears sensitive master password buffers in memory using the `zeroize` crate.
- **Zero Disk Exposure**: Secrets flow directly from memory to child process environments without temporary files.

## Acknowledgments

`envault` draws inspiration from two open-source projects:

- [cloakenv](https://github.com/hoomanaskari/cloakenv) — Encrypted local vault design, master password key derivation, and direct process injection.
- [varlock](https://github.com/dmno-dev/varlock) — Environment variable schema validation and the `@env-spec` decorator standard.

## License

[MIT](./LICENSE-MIT)
