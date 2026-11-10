use std::process::Command;
use envault::cli::audit::{format_audit_log, handle_audit};
use envault::cli::run::handle_run;
use envault::cli::validate::{format_validation_output, handle_validate};
use envault::vault::VaultClient;
use tempfile::tempdir;

#[test]
fn test_programmatic_run_injects_environment_variables() {
    let temp = tempdir().expect("tempdir should be created");
    let db_path = temp.path().join("vault.db");
    let project = "test-proj-run-env";
    let scope = "default";
    let password = "test-password-123";

    let client = VaultClient::open(&db_path, project, scope, password).unwrap();
    client.set("SERVICE_NAME", "order-service").unwrap();
    client.set("PORT", "9090").unwrap();
    client.set("DB_PASS", "super_secret_db_pass").unwrap();

    let out_file = temp.path().join("env_dump.txt");

    #[cfg(unix)]
    {
        let script = format!(
            "printf '%s|%s|%s' \"$SERVICE_NAME\" \"$PORT\" \"$DB_PASS\" > {}",
            out_file.display()
        );
        let code = handle_run(
            &db_path,
            project,
            scope,
            password,
            &["sh".to_string(), "-c".to_string(), script],
        )
        .expect("handle_run should succeed");

        assert_eq!(code, 0);

        let output = std::fs::read_to_string(&out_file).expect("file should exist");
        assert_eq!(output, "order-service|9090|super_secret_db_pass");
    }
}

#[test]
fn test_programmatic_run_exit_code_propagation() {
    let temp = tempdir().expect("tempdir should be created");
    let db_path = temp.path().join("vault.db");
    let project = "test-proj-run-exit-code";
    let scope = "default";
    let password = "test-password-123";

    // Initialize vault
    let _client = VaultClient::open(&db_path, project, scope, password).unwrap();

    #[cfg(unix)]
    {
        // Success command exits 0
        let code_success = handle_run(
            &db_path,
            project,
            scope,
            password,
            &["sh".to_string(), "-c".to_string(), "exit 0".to_string()],
        )
        .unwrap();
        assert_eq!(code_success, 0);

        // Command with exit code 42
        let code_custom = handle_run(
            &db_path,
            project,
            scope,
            password,
            &["sh".to_string(), "-c".to_string(), "exit 42".to_string()],
        )
        .unwrap();
        assert_eq!(code_custom, 42);
    }
}

#[test]
fn test_programmatic_validate_valid_and_missing_secrets() {
    let temp = tempdir().expect("tempdir should be created");
    let db_path = temp.path().join("vault.db");
    let project = "test-proj-validate-status";
    let scope = "development";
    let password = "test-password-123";

    let schema_path = temp.path().join(".env.schema");
    std::fs::write(
        &schema_path,
        r#"
# @type=url
# @required
DATABASE_URL=

# @type=string
# @required
STRIPE_SECRET_KEY=

# @type=port
API_PORT=3000
"#,
    )
    .unwrap();

    let client = VaultClient::open(&db_path, project, scope, password).unwrap();

    // 1. Initially secrets are missing -> validation returns false
    let is_valid = handle_validate(
        &db_path,
        project,
        scope,
        password,
        Some(&schema_path),
    )
    .expect("handle_validate should execute");
    assert!(!is_valid, "validation should fail when required secrets are missing");

    // 2. Set only DATABASE_URL, STRIPE_SECRET_KEY still missing
    client
        .set("DATABASE_URL", "https://postgres.database.internal:5432/main")
        .unwrap();
    let is_valid2 = handle_validate(
        &db_path,
        project,
        scope,
        password,
        Some(&schema_path),
    )
    .unwrap();
    assert!(!is_valid2, "validation should fail when STRIPE_SECRET_KEY is missing");

    // 3. Set STRIPE_SECRET_KEY, API_PORT is optional with default in schema
    client.set("STRIPE_SECRET_KEY", "sk_test_123456").unwrap();
    let is_valid3 = handle_validate(
        &db_path,
        project,
        scope,
        password,
        Some(&schema_path),
    )
    .unwrap();
    assert!(is_valid3, "validation should pass when all required secrets are set");

    // 4. Set invalid API_PORT value
    client.set("API_PORT", "not-a-port").unwrap();
    let is_valid4 = handle_validate(
        &db_path,
        project,
        scope,
        password,
        Some(&schema_path),
    )
    .unwrap();
    assert!(!is_valid4, "validation should fail when API_PORT is invalid");
}

#[test]
fn test_programmatic_validate_formatting() {
    let entries = vec![
        envault::schema::SchemaEntry {
            key: "DATABASE_URL".to_string(),
            schema_type: envault::schema::SchemaType::Url,
            required: true,
            sensitive: true,
            default_value: None,
            description: None,
        },
        envault::schema::SchemaEntry {
            key: "STRIPE_SECRET_KEY".to_string(),
            schema_type: envault::schema::SchemaType::String,
            required: true,
            sensitive: true,
            default_value: None,
            description: None,
        },
        envault::schema::SchemaEntry {
            key: "APP_ENV".to_string(),
            schema_type: envault::schema::SchemaType::String,
            required: false,
            sensitive: false,
            default_value: None,
            description: None,
        },
        envault::schema::SchemaEntry {
            key: "API_PORT".to_string(),
            schema_type: envault::schema::SchemaType::Port,
            required: true,
            sensitive: false,
            default_value: None,
            description: None,
        },
    ];

    let errors = vec![
        envault::schema::ValidationError::new(
            "STRIPE_SECRET_KEY",
            "required",
            "Secret is required but not set",
        ),
        envault::schema::ValidationError::new(
            "API_PORT",
            "port",
            "expected port (1-65535), got \"not_a_port\"",
        ),
    ];

    let formatted = format_validation_output("my-app", "development", &entries, &errors);
    assert!(formatted.contains("Validating project: my-app (scope: development)"));
    assert!(formatted.contains("KEY                  STATUS    DETAIL"));
    assert!(formatted.contains("DATABASE_URL         ✓ OK"));
    assert!(formatted.contains("STRIPE_SECRET_KEY    ✗ MISSING  required"));
    assert!(formatted.contains("APP_ENV              ✓ OK"));
    assert!(formatted.contains("API_PORT             ✗ INVALID  expected port (1-65535), got \"not_a_port\""));
    assert!(formatted.contains("2 errors found."));
}

#[test]
fn test_programmatic_audit_records_actions() {
    let temp = tempdir().expect("tempdir should be created");
    let db_path = temp.path().join("vault.db");
    let project = "test-proj-audit-actions";
    let scope = "staging";
    let password = "test-password-123";

    let client = VaultClient::open(&db_path, project, scope, password).unwrap();

    // 1. Write an action
    client.set("AUTH_SECRET", "super_auth").unwrap();
    // 2. Read action
    let _ = client.get("AUTH_SECRET").unwrap();
    // 3. Delete action
    client.delete("AUTH_SECRET").unwrap();

    let logs = client.get_audit_log().unwrap();
    assert_eq!(logs.len(), 3);
    assert_eq!(logs[0].action, envault::vault::AuditAction::Delete);
    assert_eq!(logs[1].action, envault::vault::AuditAction::Read);
    assert_eq!(logs[2].action, envault::vault::AuditAction::Write);

    let table = format_audit_log(&logs);
    assert!(table.contains("Time                  Scope         Action   Key"));
    assert!(table.contains("staging"));
    assert!(table.contains("DELETE"));
    assert!(table.contains("READ"));
    assert!(table.contains("WRITE"));
    assert!(table.contains("AUTH_SECRET"));

    // handle_audit should succeed cleanly
    handle_audit(&db_path, project, scope, password).expect("handle_audit should succeed");
}

#[test]
fn test_cli_binary_run_e2e() {
    let bin_path = env!("CARGO_BIN_EXE_envault");
    let temp = tempdir().expect("tempdir should be created");
    let db_path = temp.path().join("vault.db");
    let project = "test-proj-bin-run";
    let scope = "prod";
    let password = "bin-password-e2e";

    // Set a secret first
    let set_output = Command::new(bin_path)
        .env("ENVAULT_PASSWORD", password)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--project",
            project,
            "--scope",
            scope,
            "set",
            "INJECTED_VAR=it_works_seamlessly",
        ])
        .output()
        .expect("set command should execute");
    assert!(set_output.status.success());

    // Run a command injecting the secret
    #[cfg(unix)]
    {
        let run_output = Command::new(bin_path)
            .env("ENVAULT_PASSWORD", password)
            .args([
                "--db",
                db_path.to_str().unwrap(),
                "--project",
                project,
                "--scope",
                scope,
                "run",
                "--",
                "sh",
                "-c",
                "printf '%s' \"$INJECTED_VAR\"",
            ])
            .output()
            .expect("run command should execute");

        assert!(run_output.status.success());
        let stdout = String::from_utf8_lossy(&run_output.stdout);
        assert_eq!(stdout, "it_works_seamlessly");

        // Test non-zero exit code propagation
        let exit_code_output = Command::new(bin_path)
            .env("ENVAULT_PASSWORD", password)
            .args([
                "--db",
                db_path.to_str().unwrap(),
                "--project",
                project,
                "--scope",
                scope,
                "run",
                "--",
                "sh",
                "-c",
                "exit 37",
            ])
            .output()
            .expect("run command with non-zero exit should execute");

        assert_eq!(exit_code_output.status.code(), Some(37));
    }
}

#[test]
fn test_cli_binary_validate_e2e() {
    let bin_path = env!("CARGO_BIN_EXE_envault");
    let temp = tempdir().expect("tempdir should be created");
    let db_path = temp.path().join("vault.db");
    let project = "test-proj-bin-validate";
    let scope = "default";
    let password = "bin-password-e2e";

    let schema_path = temp.path().join(".env.schema");
    std::fs::write(
        &schema_path,
        r#"
# @type=url
# @required
SERVICE_URL=
"#,
    )
    .unwrap();

    // 1. Validate when secret is missing -> exit code 1
    let val_output_fail = Command::new(bin_path)
        .env("ENVAULT_PASSWORD", password)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--project",
            project,
            "--scope",
            scope,
            "validate",
            "--schema",
            schema_path.to_str().unwrap(),
        ])
        .output()
        .expect("validate command should execute");

    assert_eq!(val_output_fail.status.code(), Some(1));
    let stdout_fail = String::from_utf8_lossy(&val_output_fail.stdout);
    assert!(stdout_fail.contains("SERVICE_URL"));
    assert!(stdout_fail.contains("MISSING"));

    // 2. Set the secret
    let set_output = Command::new(bin_path)
        .env("ENVAULT_PASSWORD", password)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--project",
            project,
            "--scope",
            scope,
            "set",
            "SERVICE_URL=https://api.example.com",
        ])
        .output()
        .expect("set command should execute");
    assert!(set_output.status.success());

    // 3. Validate again -> success (exit code 0)
    let val_output_success = Command::new(bin_path)
        .env("ENVAULT_PASSWORD", password)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--project",
            project,
            "--scope",
            scope,
            "validate",
            "--schema",
            schema_path.to_str().unwrap(),
        ])
        .output()
        .expect("validate command should execute");

    assert_eq!(val_output_success.status.code(), Some(0));
    let stdout_success = String::from_utf8_lossy(&val_output_success.stdout);
    assert!(stdout_success.contains("SERVICE_URL"));
    assert!(stdout_success.contains("✓ OK"));
}

#[test]
fn test_cli_binary_audit_e2e() {
    let bin_path = env!("CARGO_BIN_EXE_envault");
    let temp = tempdir().expect("tempdir should be created");
    let db_path = temp.path().join("vault.db");
    let project = "test-proj-bin-audit";
    let scope = "default";
    let password = "bin-password-e2e";

    // Perform an operation that creates an audit entry
    let set_output = Command::new(bin_path)
        .env("ENVAULT_PASSWORD", password)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--project",
            project,
            "--scope",
            scope,
            "set",
            "AUDIT_KEY=audit_val",
        ])
        .output()
        .expect("set command should execute");
    assert!(set_output.status.success());

    // Call audit command
    let audit_output = Command::new(bin_path)
        .env("ENVAULT_PASSWORD", password)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--project",
            project,
            "--scope",
            scope,
            "audit",
        ])
        .output()
        .expect("audit command should execute");

    assert!(audit_output.status.success());
    let stdout = String::from_utf8_lossy(&audit_output.stdout);
    assert!(stdout.contains("Time                  Scope         Action   Key"));
    assert!(stdout.contains("WRITE"));
    assert!(stdout.contains("AUDIT_KEY"));
}
