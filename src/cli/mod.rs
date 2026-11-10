use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::{Parser, Subcommand};

use crate::vault::VaultClient;

pub mod audit;
pub mod run;
pub mod validate;

pub use audit::handle_audit;
pub use run::handle_run;
pub use validate::handle_validate;

/// Command-line arguments for envault.
#[derive(Parser, Debug, Clone)]
#[command(
    name = "envault",
    version,
    about = "Encrypted environment secret manager with schema enforcement",
    long_about = None
)]
pub struct Cli {
    /// Override the detected project name
    #[arg(long, global = true)]
    pub project: Option<String>,

    /// Environment scope
    #[arg(long, global = true, default_value = "default")]
    pub scope: String,

    /// Vault database path [default: ~/.config/envault/vault.db]
    #[arg(long, global = true)]
    pub db: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

pub type Command = Commands;

/// Available subcommands for envault CLI.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum Commands {
    /// Store an encrypted secret (KEY=VALUE)
    Set {
        /// Key-value pair in KEY=VALUE format
        key_val: String,
    },
    /// Retrieve and decrypt a secret by key
    Get {
        /// Secret key to retrieve
        key: String,
    },
    /// List all secret keys for the current project and scope
    List,
    /// Delete a secret by key
    #[command(alias = "rm")]
    Delete {
        /// Secret key to delete
        key: String,
    },
    /// Run a command with decrypted secrets injected as environment variables
    Run {
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },
    /// Validate stored secrets against a schema file
    Validate {
        #[arg(long)]
        schema: Option<PathBuf>,
    },
    /// Print the audit log for the current project
    Audit,
}

/// Expands `~` or `~/...` in a path using the user's home directory.
pub fn expand_tilde(path: &Path) -> anyhow::Result<PathBuf> {
    let path_str = path.to_string_lossy();
    if path_str == "~" {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("failed to determine home directory for tilde expansion")?;
        return Ok(PathBuf::from(home));
    }
    if let Some(stripped) = path_str.strip_prefix("~/") {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("failed to determine home directory for tilde expansion")?;
        return Ok(PathBuf::from(home).join(stripped));
    }
    #[cfg(windows)]
    if let Some(stripped) = path_str.strip_prefix("~\\") {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("failed to determine home directory for tilde expansion")?;
        return Ok(PathBuf::from(home).join(stripped));
    }
    Ok(path.to_path_buf())
}

/// Resolves the vault database path.
/// If `custom_db` is provided, expands `~` if needed.
/// If `custom_db` is `None`, falls back to `crate::vault::default_db_path()`.
/// Ensures the parent directory exists.
pub fn resolve_db_path(custom_db: Option<&Path>) -> anyhow::Result<PathBuf> {
    let path = match custom_db {
        Some(p) => expand_tilde(p)?,
        None => crate::vault::default_db_path()?,
    };

    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create database directory at {}", parent.display()))?;
    }

    Ok(path)
}

/// Resolves the project name using current working directory if `project` is not provided.
pub fn resolve_project_name(project: Option<&str>) -> anyhow::Result<String> {
    let cwd = std::env::current_dir().context("failed to determine current working directory")?;
    resolve_project_name_from(project, &cwd)
}

/// Resolves the project name using `start_dir` for project detection if `project` is not provided.
pub fn resolve_project_name_from(project: Option<&str>, start_dir: &Path) -> anyhow::Result<String> {
    if let Some(trimmed) = project.map(str::trim).filter(|p| !p.is_empty()) {
        return Ok(trimmed.to_string());
    }
    let info = crate::project::detect_project(start_dir)
        .with_context(|| format!("failed to detect project from {}", start_dir.display()))?;
    Ok(info.name)
}

/// Resolves the master password:
/// 1. Checks `ENVAULT_PASSWORD` environment variable first.
/// 2. If stdin is interactive (a terminal), prompts the user.
/// 3. In test/headless environments (non-interactive stdin), falls back to a default password.
pub fn resolve_password() -> anyhow::Result<String> {
    if let Some(password) = std::env::var("ENVAULT_PASSWORD").ok().filter(|p| !p.is_empty()) {
        return Ok(password);
    }

    if std::io::stdin().is_terminal() {
        eprint!("Enter master password: ");
        std::io::stderr().flush().context("failed to flush stderr")?;
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .context("failed to read password from stdin")?;
        let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
        if trimmed.is_empty() {
            anyhow::bail!("master password cannot be empty");
        }
        Ok(trimmed.to_string())
    } else {
        Ok("envault-default-password".to_string())
    }
}

/// Handles the `set` subcommand.
pub fn handle_set(
    db_path: &Path,
    project: &str,
    scope: &str,
    password: &str,
    key_val: &str,
) -> anyhow::Result<()> {
    let (key, value) = key_val
        .split_once('=')
        .ok_or_else(|| anyhow::anyhow!("invalid secret format '{}', expected KEY=VALUE", key_val))?;

    let key = key.trim();
    if key.is_empty() {
        anyhow::bail!("secret key cannot be empty");
    }

    let client = VaultClient::open(db_path, project, scope, password)
        .with_context(|| format!("failed to open vault database at {}", db_path.display()))?;
    client.set(key, value)?;
    Ok(())
}

/// Retrieves a secret value without printing it (useful for programmatic testing).
pub fn get_secret_value(
    db_path: &Path,
    project: &str,
    scope: &str,
    password: &str,
    key: &str,
) -> anyhow::Result<String> {
    let client = VaultClient::open(db_path, project, scope, password)
        .with_context(|| format!("failed to open vault database at {}", db_path.display()))?;

    client
        .get(key)?
        .ok_or_else(|| anyhow::anyhow!("secret '{key}' not found"))
}

/// Handles the `get` subcommand by printing the decrypted plaintext value to stdout.
pub fn handle_get(
    db_path: &Path,
    project: &str,
    scope: &str,
    password: &str,
    key: &str,
) -> anyhow::Result<()> {
    let value = get_secret_value(db_path, project, scope, password, key)?;
    println!("{value}");
    Ok(())
}

/// Retrieves list of secret keys for current project and scope.
pub fn list_secret_keys(
    db_path: &Path,
    project: &str,
    scope: &str,
    password: &str,
) -> anyhow::Result<Vec<String>> {
    let client = VaultClient::open(db_path, project, scope, password)
        .with_context(|| format!("failed to open vault database at {}", db_path.display()))?;
    client.list()
}

/// Handles the `list` subcommand by printing stored secret keys to stdout.
pub fn handle_list(
    db_path: &Path,
    project: &str,
    scope: &str,
    password: &str,
) -> anyhow::Result<()> {
    let keys = list_secret_keys(db_path, project, scope, password)?;
    for key in keys {
        println!("{key}");
    }
    Ok(())
}

/// Handles the `delete` subcommand by deleting the specified secret.
pub fn handle_delete(
    db_path: &Path,
    project: &str,
    scope: &str,
    password: &str,
    key: &str,
) -> anyhow::Result<()> {
    let client = VaultClient::open(db_path, project, scope, password)
        .with_context(|| format!("failed to open vault database at {}", db_path.display()))?;
    client.delete(key)?;
    Ok(())
}

/// Dispatches CLI command according to parsed arguments.
pub fn run_cli(args: Cli) -> anyhow::Result<()> {
    let command = match args.command {
        Some(cmd) => cmd,
        None => return Ok(()),
    };

    let db_path = resolve_db_path(args.db.as_deref())?;
    let project = resolve_project_name(args.project.as_deref())?;
    let scope = args.scope;
    let password = resolve_password()?;

    match command {
        Commands::Set { key_val } => {
            handle_set(&db_path, &project, &scope, &password, &key_val)?;
        }
        Commands::Get { key } => {
            handle_get(&db_path, &project, &scope, &password, &key)?;
        }
        Commands::List => {
            handle_list(&db_path, &project, &scope, &password)?;
        }
        Commands::Delete { key } => {
            handle_delete(&db_path, &project, &scope, &password, &key)?;
        }
        Commands::Run { command } => {
            let code = run::handle_run(&db_path, &project, &scope, &password, &command)?;
            if code != 0 {
                if std::env::var("ENVAULT_NO_EXIT").is_ok() {
                    anyhow::bail!("child process exited with code {}", code);
                }
                std::process::exit(code);
            }
        }
        Commands::Validate { schema } => {
            let ok = validate::handle_validate(
                &db_path,
                &project,
                &scope,
                &password,
                schema.as_deref(),
            )?;
            if !ok {
                if std::env::var("ENVAULT_NO_EXIT").is_ok() {
                    anyhow::bail!("schema validation failed");
                }
                std::process::exit(1);
            }
        }
        Commands::Audit => {
            audit::handle_audit(&db_path, &project, &scope, &password)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_clap_parse_subcommands() {
        let args = Cli::parse_from(["envault", "set", "FOO=bar"]);
        assert_eq!(
            args.command,
            Some(Commands::Set {
                key_val: "FOO=bar".to_string()
            })
        );
        assert_eq!(args.scope, "default");
        assert_eq!(args.project, None);
        assert_eq!(args.db, None);

        let args = Cli::parse_from(["envault", "get", "FOO"]);
        assert_eq!(
            args.command,
            Some(Commands::Get {
                key: "FOO".to_string()
            })
        );

        let args = Cli::parse_from(["envault", "list"]);
        assert_eq!(args.command, Some(Commands::List));

        let args = Cli::parse_from(["envault", "delete", "FOO"]);
        assert_eq!(
            args.command,
            Some(Commands::Delete {
                key: "FOO".to_string()
            })
        );

        let args = Cli::parse_from(["envault", "run", "--", "echo", "hello", "world"]);
        assert_eq!(
            args.command,
            Some(Commands::Run {
                command: vec!["echo".to_string(), "hello".to_string(), "world".to_string()]
            })
        );

        let args = Cli::parse_from(["envault", "validate", "--schema", "/path/to/.env.schema"]);
        assert_eq!(
            args.command,
            Some(Commands::Validate {
                schema: Some(PathBuf::from("/path/to/.env.schema"))
            })
        );

        let args = Cli::parse_from(["envault", "audit"]);
        assert_eq!(args.command, Some(Commands::Audit));
    }

    #[test]
    fn test_clap_parse_global_flags() {
        let args = Cli::parse_from([
            "envault",
            "--project",
            "my-project",
            "--scope",
            "staging",
            "--db",
            "/tmp/custom.db",
            "set",
            "KEY=VALUE",
        ]);
        assert_eq!(args.project.as_deref(), Some("my-project"));
        assert_eq!(args.scope, "staging");
        assert_eq!(args.db, Some(PathBuf::from("/tmp/custom.db")));
        assert_eq!(
            args.command,
            Some(Commands::Set {
                key_val: "KEY=VALUE".to_string()
            })
        );
    }

    #[test]
    fn test_resolve_project_name() {
        let explicit = resolve_project_name(Some("custom-proj")).unwrap();
        assert_eq!(explicit, "custom-proj");

        let temp = tempdir().unwrap();
        let proj_dir = temp.path().join("detected-proj");
        std::fs::create_dir(&proj_dir).unwrap();
        std::fs::write(proj_dir.join("Cargo.toml"), "[package]\nname = \"detected-proj\"").unwrap();

        let detected = resolve_project_name_from(None, &proj_dir).unwrap();
        assert_eq!(detected, "detected-proj");
    }

    #[test]
    fn test_resolve_db_path_and_mkdir() {
        let temp = tempdir().unwrap();
        let nested_db = temp.path().join("nested").join("dir").join("test.db");
        assert!(!nested_db.parent().unwrap().exists());

        let resolved = resolve_db_path(Some(&nested_db)).unwrap();
        assert_eq!(resolved, nested_db);
        assert!(nested_db.parent().unwrap().exists());
    }

    #[test]
    fn test_handle_set_and_get_value() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("test.db");
        let project = "test-project-set-get";
        let scope = "default";
        let password = "pass";

        handle_set(&db_path, project, scope, password, "DATABASE_URL=postgres://localhost:5432/db")
            .unwrap();

        let val = get_secret_value(&db_path, project, scope, password, "DATABASE_URL").unwrap();
        assert_eq!(val, "postgres://localhost:5432/db");

        // Non-existent key returns error
        let err = get_secret_value(&db_path, project, scope, password, "NON_EXISTENT");
        assert!(err.is_err());
    }

    #[test]
    fn test_handle_list_and_delete() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("test.db");
        let project = "test-project-list-delete";
        let scope = "default";
        let password = "pass";

        handle_set(&db_path, project, scope, password, "B_KEY=b").unwrap();
        handle_set(&db_path, project, scope, password, "A_KEY=a").unwrap();
        handle_set(&db_path, project, scope, password, "C_KEY=c").unwrap();

        let keys = list_secret_keys(&db_path, project, scope, password).unwrap();
        assert_eq!(keys, vec!["A_KEY", "B_KEY", "C_KEY"]);

        handle_delete(&db_path, project, scope, password, "B_KEY").unwrap();

        let keys_after = list_secret_keys(&db_path, project, scope, password).unwrap();
        assert_eq!(keys_after, vec!["A_KEY", "C_KEY"]);
    }

    #[test]
    fn test_handle_set_invalid_format() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("test.db");

        let err = handle_set(&db_path, "proj", "default", "pass", "NO_EQUALS_SIGN");
        assert!(err.is_err());

        let err2 = handle_set(&db_path, "proj", "default", "pass", "=NO_KEY");
        assert!(err2.is_err());
    }
}
