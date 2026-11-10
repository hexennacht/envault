use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// Global configuration loaded from `~/.config/envault/config.toml` (SPEC Table 4.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Config {
    /// Default environment scope when `--scope` is omitted (e.g. "development", "staging")
    #[serde(default, alias = "scope")]
    pub default_scope: Option<String>,

    /// Default database path override
    #[serde(default, alias = "db_path", alias = "database")]
    pub db: Option<PathBuf>,
}

/// Returns the default path for `config.toml`: `~/.config/envault/config.toml`.
pub fn default_config_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("failed to determine user home directory")?;

    Ok(PathBuf::from(home)
        .join(".config")
        .join("envault")
        .join("config.toml"))
}

/// Attempts to load optional configuration from `~/.config/envault/config.toml`.
/// Returns `Ok(None)` if the file does not exist or user home cannot be determined.
pub fn load_config() -> anyhow::Result<Option<Config>> {
    let path = match default_config_path() {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };
    load_config_from(&path)
}

/// Attempts to load configuration from the specified path.
/// Returns `Ok(None)` if the file does not exist.
pub fn load_config_from(path: &Path) -> anyhow::Result<Option<Config>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config file from {}", path.display()))?;
    let config: Config = toml::from_str(&content)
        .with_context(|| format!("failed to parse toml config from {}", path.display()))?;
    Ok(Some(config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_config_non_existent() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let result = load_config_from(&path).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_load_config_valid() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
default_scope = "production"
db = "/custom/path/to/vault.db"
"#,
        )
        .unwrap();

        let config = load_config_from(&path).unwrap().expect("config should be parsed");
        assert_eq!(config.default_scope.as_deref(), Some("production"));
        assert_eq!(config.db, Some(PathBuf::from("/custom/path/to/vault.db")));
    }

    #[test]
    fn test_load_config_aliases() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
scope = "staging"
db_path = "~/my_vault.db"
"#,
        )
        .unwrap();

        let config = load_config_from(&path).unwrap().expect("config should be parsed");
        assert_eq!(config.default_scope.as_deref(), Some("staging"));
        assert_eq!(config.db, Some(PathBuf::from("~/my_vault.db")));
    }

    #[test]
    fn test_load_config_invalid_toml() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        std::fs::write(&path, "this is not = = toml").unwrap();

        let result = load_config_from(&path);
        assert!(result.is_err());
    }
}
