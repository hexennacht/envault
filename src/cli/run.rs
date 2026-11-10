use std::path::Path;
use anyhow::Context;

use crate::vault::VaultClient;

/// Handles the `run` subcommand by decrypting all secrets for the given project and scope,
/// merging schema default values, injecting them into the child process environment,
/// spawning the command, and returning its exit status code.
pub fn handle_run<'a>(
    db_path: &Path,
    project: &str,
    scope: &str,
    password: impl Into<Option<&'a str>>,
    command: &[String],
) -> anyhow::Result<i32> {
    handle_run_with_schema(db_path, project, scope, password, command, None)
}

/// Handles the `run` subcommand with an optional explicit schema path.
pub fn handle_run_with_schema<'a>(
    db_path: &Path,
    project: &str,
    scope: &str,
    password: impl Into<Option<&'a str>>,
    command: &[String],
    custom_schema_path: Option<&Path>,
) -> anyhow::Result<i32> {
    if command.is_empty() {
        anyhow::bail!("no command specified to run");
    }

    let client = VaultClient::open(db_path, project, scope, password)
        .with_context(|| format!("failed to open vault database at {}", db_path.display()))?;
    let secrets = client
        .get_all()
        .with_context(|| format!("failed to retrieve secrets for project '{project}', scope '{scope}'"))?;

    let mut env_vars = std::collections::HashMap::new();

    // Merge schema default values for keys that are not present in the vault
    if let Ok(Some(schema_path)) = crate::cli::validate::resolve_schema_path(custom_schema_path)
        && let Ok(schema_entries) = crate::schema::parser::parse_schema_file(&schema_path)
    {
        for entry in schema_entries {
            if let Some(default_val) = entry.default_value {
                env_vars.insert(entry.key, default_val);
            }
        }
    }

    // Vault secrets override schema defaults
    for (key, val) in secrets {
        env_vars.insert(key, val);
    }

    let program = &command[0];
    let args = &command[1..];

    let mut cmd = std::process::Command::new(program);
    cmd.args(args);
    for (key, val) in env_vars {
        cmd.env(key, val);
    }
    cmd.stdin(std::process::Stdio::inherit());
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::inherit());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn child process '{program}'"))?;
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for child process '{program}'"))?;

    let exit_code = match status.code() {
        Some(code) => code,
        None => {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                status.signal().map(|s| 128 + s).unwrap_or(1)
            }
            #[cfg(not(unix))]
            {
                1
            }
        }
    };

    Ok(exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_handle_run_empty_command() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("vault.db");
        let result = handle_run(&db_path, "test", "default", "pass", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_run_command_execution() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("vault.db");
        let project = "test-proj-run-cmd-exec";
        let client = VaultClient::open(&db_path, project, "default", "pass").unwrap();
        client.set("RUN_TEST_KEY", "run_test_val").unwrap();

        // Run a simple command: true (should exit 0)
        #[cfg(unix)]
        {
            let code = handle_run(
                &db_path,
                project,
                "default",
                "pass",
                &["true".to_string()],
            )
            .unwrap();
            assert_eq!(code, 0);

            // Run false (should exit 1)
            let code = handle_run(
                &db_path,
                project,
                "default",
                "pass",
                &["false".to_string()],
            )
            .unwrap();
            assert_eq!(code, 1);
        }
    }

    #[test]
    fn test_handle_run_injects_env() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("vault.db");
        let out_file = temp.path().join("env_out.txt");
        let project = "test-proj-run-inject-env";

        let client = VaultClient::open(&db_path, project, "default", "pass").unwrap();
        client.set("MY_SECRET_VAR", "super_secret_42").unwrap();

        #[cfg(unix)]
        {
            let script = format!("printf '%s' \"$MY_SECRET_VAR\" > {}", out_file.display());
            let code = handle_run(
                &db_path,
                project,
                "default",
                "pass",
                &["sh".to_string(), "-c".to_string(), script],
            )
            .unwrap();
            assert_eq!(code, 0);

            let content = std::fs::read_to_string(&out_file).unwrap();
            assert_eq!(content, "super_secret_42");
        }
    }
}
