use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::schema::parser::parse_schema_file;
use crate::schema::types::SchemaEntry;
use crate::schema::validator::{validate, ValidationError};
use crate::vault::VaultClient;

/// Resolves the path to the schema file:
///
/// 1. Uses `schema_path` if explicitly provided (fails if it doesn't exist).
/// 2. Checks current working directory for `.env.schema`.
/// 3. Checks project root (via project detection) for `.env.schema`.
///
/// Returns `Ok(None)` if no schema file is found.
pub fn resolve_schema_path(schema_path: Option<&Path>) -> anyhow::Result<Option<PathBuf>> {
    if let Some(explicit) = schema_path {
        if explicit.exists() {
            return Ok(Some(explicit.to_path_buf()));
        } else {
            anyhow::bail!("schema file not found at '{}'", explicit.display());
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        let cwd_schema = cwd.join(".env.schema");
        if cwd_schema.exists() {
            return Ok(Some(cwd_schema));
        }

        if let Ok(info) = crate::project::detect_project(&cwd) {
            let root_schema = info.root.join(".env.schema");
            if root_schema.exists() {
                return Ok(Some(root_schema));
            }
        }
    }

    Ok(None)
}

/// Formats the validation results table according to SPEC §9.4.
pub fn format_validation_output(
    project: &str,
    scope: &str,
    entries: &[SchemaEntry],
    errors: &[ValidationError],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("Validating project: {project} (scope: {scope})\n\n"));

    let mut error_map: HashMap<&str, &ValidationError> = HashMap::new();
    for err in errors {
        error_map.entry(&err.key).or_insert(err);
    }

    let key_width = entries.iter().map(|e| e.key.len()).max().unwrap_or(3).max(19);
    out.push_str(&format!("  {:<key_width$}  STATUS    DETAIL\n", "KEY"));

    for entry in entries {
        if let Some(err) = error_map.get(entry.key.as_str()) {
            if err.rule == "required" {
                out.push_str(&format!("  {:<key_width$}  ✗ MISSING  required\n", entry.key));
            } else {
                out.push_str(&format!(
                    "  {:<key_width$}  ✗ INVALID  {}\n",
                    entry.key, err.message
                ));
            }
        } else {
            out.push_str(&format!("  {:<key_width$}  ✓ OK\n", entry.key));
        }
    }

    let error_count = errors.len();
    if error_count == 0 {
        out.push_str("\n0 errors found.\n");
    } else if error_count == 1 {
        out.push_str("\n1 error found.\n");
    } else {
        out.push_str(&format!("\n{} errors found.\n", error_count));
    }

    out
}

/// Handles the `validate` subcommand:
/// - Resolves and parses `.env.schema`.
/// - Decrypts all secrets for the project and scope.
/// - Validates secrets against schema rules.
/// - Prints the validation table.
/// - Returns `Ok(true)` if 0 errors, `Ok(false)` if errors found.
pub fn handle_validate<'a>(
    db_path: &Path,
    project: &str,
    scope: &str,
    password: impl Into<Option<&'a str>>,
    schema_path: Option<&Path>,
) -> anyhow::Result<bool> {
    let resolved = resolve_schema_path(schema_path)?;
    let schema_file = match resolved {
        Some(p) => p,
        None => {
            println!("No .env.schema found. Validation skipped.");
            return Ok(true);
        }
    };

    let entries = parse_schema_file(&schema_file)
        .with_context(|| format!("failed to parse schema file at {}", schema_file.display()))?;

    let client = VaultClient::open(db_path, project, scope, password)
        .with_context(|| format!("failed to open vault database at {}", db_path.display()))?;
    let secrets = client
        .get_all()
        .with_context(|| format!("failed to retrieve secrets for project '{project}', scope '{scope}'"))?;

    let errors = validate(&entries, &secrets);
    let output = format_validation_output(project, scope, &entries, &errors);
    print!("{output}");

    Ok(errors.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_format_validation_output_all_ok() {
        let entries = vec![
            SchemaEntry {
                key: "DATABASE_URL".to_string(),
                schema_type: crate::schema::types::SchemaType::Url,
                required: true,
                sensitive: true,
                default_value: None,
                description: None,
            },
            SchemaEntry {
                key: "APP_ENV".to_string(),
                schema_type: crate::schema::types::SchemaType::String,
                required: false,
                sensitive: false,
                default_value: None,
                description: None,
            },
        ];
        let errors = vec![];
        let out = format_validation_output("my-app", "development", &entries, &errors);

        assert!(out.contains("Validating project: my-app (scope: development)"));
        assert!(out.contains("KEY                  STATUS    DETAIL"));
        assert!(out.contains("DATABASE_URL         ✓ OK"));
        assert!(out.contains("APP_ENV              ✓ OK"));
        assert!(out.contains("0 errors found."));
    }

    #[test]
    fn test_format_validation_output_with_errors() {
        let entries = vec![
            SchemaEntry {
                key: "DATABASE_URL".to_string(),
                schema_type: crate::schema::types::SchemaType::Url,
                required: true,
                sensitive: true,
                default_value: None,
                description: None,
            },
            SchemaEntry {
                key: "STRIPE_SECRET_KEY".to_string(),
                schema_type: crate::schema::types::SchemaType::String,
                required: true,
                sensitive: true,
                default_value: None,
                description: None,
            },
            SchemaEntry {
                key: "API_PORT".to_string(),
                schema_type: crate::schema::types::SchemaType::Port,
                required: true,
                sensitive: false,
                default_value: None,
                description: None,
            },
        ];
        let errors = vec![
            ValidationError::new("STRIPE_SECRET_KEY", "required", "Secret is required but not set"),
            ValidationError::new("API_PORT", "port", "expected port (1-65535), got \"not_a_port\""),
        ];
        let out = format_validation_output("my-app", "development", &entries, &errors);

        assert!(out.contains("DATABASE_URL         ✓ OK"));
        assert!(out.contains("STRIPE_SECRET_KEY    ✗ MISSING  required"));
        assert!(out.contains("API_PORT             ✗ INVALID  expected port (1-65535), got \"not_a_port\""));
        assert!(out.contains("2 errors found."));
    }

    #[test]
    fn test_handle_validate_end_to_end() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("vault.db");
        let schema_path = temp.path().join(".env.schema");
        let project = "test-proj-validate-e2e";

        std::fs::write(
            &schema_path,
            "# @type=url\n# @required\nDATABASE_URL=\n\n# @type=port\n# @required\nPORT=\n",
        )
        .unwrap();

        let client = VaultClient::open(&db_path, project, "default", "pass").unwrap();
        // Set valid DATABASE_URL and invalid PORT
        client.set("DATABASE_URL", "https://postgres.local:5432/db").unwrap();
        client.set("PORT", "not_a_port").unwrap();

        let result = handle_validate(
            &db_path,
            project,
            "default",
            "pass",
            Some(&schema_path),
        )
        .unwrap();
        assert!(!result, "validation should fail due to invalid port");

        // Fix PORT
        client.set("PORT", "8080").unwrap();
        let result_fixed = handle_validate(
            &db_path,
            project,
            "default",
            "pass",
            Some(&schema_path),
        )
        .unwrap();
        assert!(result_fixed, "validation should pass once all secrets are valid");
    }

    #[test]
    fn test_handle_validate_no_schema_skips() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("vault.db");
        // Non-existent schema path explicitly given should fail
        let missing = temp.path().join("nonexistent.schema");
        let res = handle_validate(&db_path, "proj", "default", "pass", Some(&missing));
        assert!(res.is_err());
    }
}
