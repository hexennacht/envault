use std::process::Command;
use envault::cli::{
    get_secret_value, handle_delete, handle_set, list_secret_keys, run_cli, Cli, Commands,
};
use tempfile::tempdir;

#[test]
fn test_cli_set_get_roundtrip_programmatic() {
    let temp = tempdir().expect("tempdir should be created");
    let db_path = temp.path().join("vault.db");
    let project = "test-proj-set-get";
    let scope = "default";
    let password = "test-master-password";

    // Set a secret
    handle_set(&db_path, project, scope, password, "MY_SECRET=hello_world")
        .expect("handle_set should succeed");

    // Get the secret
    let val = get_secret_value(&db_path, project, scope, password, "MY_SECRET")
        .expect("get_secret_value should succeed");
    assert_eq!(val, "hello_world");
}

#[test]
fn test_cli_list_keys_programmatic() {
    let temp = tempdir().expect("tempdir should be created");
    let db_path = temp.path().join("vault.db");
    let project = "test-proj-list";
    let scope = "default";
    let password = "test-master-password";

    handle_set(&db_path, project, scope, password, "B_KEY=second").unwrap();
    handle_set(&db_path, project, scope, password, "A_KEY=first").unwrap();
    handle_set(&db_path, project, scope, password, "C_KEY=third").unwrap();

    let keys = list_secret_keys(&db_path, project, scope, password).unwrap();
    assert_eq!(keys, vec!["A_KEY", "B_KEY", "C_KEY"]);
}

#[test]
fn test_cli_delete_secret_programmatic() {
    let temp = tempdir().expect("tempdir should be created");
    let db_path = temp.path().join("vault.db");
    let project = "test-proj-delete";
    let scope = "default";
    let password = "test-master-password";

    handle_set(&db_path, project, scope, password, "TO_DELETE=val").unwrap();
    handle_set(&db_path, project, scope, password, "KEEP=val2").unwrap();

    handle_delete(&db_path, project, scope, password, "TO_DELETE").unwrap();

    let keys = list_secret_keys(&db_path, project, scope, password).unwrap();
    assert_eq!(keys, vec!["KEEP"]);

    let err = get_secret_value(&db_path, project, scope, password, "TO_DELETE");
    assert!(err.is_err());
}

#[test]
fn test_cli_custom_scope_isolation() {
    let temp = tempdir().expect("tempdir should be created");
    let db_path = temp.path().join("vault.db");
    let project = "test-proj-scope";
    let password = "test-master-password";

    // Store different values for same key in different scopes
    handle_set(&db_path, project, "staging", password, "API_URL=https://staging.example.com")
        .unwrap();
    handle_set(&db_path, project, "production", password, "API_URL=https://prod.example.com")
        .unwrap();

    let staging_val = get_secret_value(&db_path, project, "staging", password, "API_URL").unwrap();
    let prod_val = get_secret_value(&db_path, project, "production", password, "API_URL").unwrap();
    assert_eq!(staging_val, "https://staging.example.com");
    assert_eq!(prod_val, "https://prod.example.com");

    // Key should not exist in default scope
    let default_val = get_secret_value(&db_path, project, "default", password, "API_URL");
    assert!(default_val.is_err());
}

#[test]
fn test_cli_run_cli_dispatch() {
    let temp = tempdir().expect("tempdir should be created");
    let db_path = temp.path().join("vault.db");
    let project = "test-proj-dispatch";
    let scope = "custom-scope";

    // Use run_cli with Commands::Set
    let set_args = Cli {
        project: Some(project.to_string()),
        scope: scope.to_string(),
        db: Some(db_path.clone()),
        command: Some(Commands::Set {
            key_val: "DISPATCH_KEY=dispatched_value".to_string(),
        }),
    };
    run_cli(set_args).expect("run_cli Set should succeed");

    // Verify stored
    let val = get_secret_value(&db_path, project, scope, "envault-default-password", "DISPATCH_KEY")
        .expect("secret should exist");
    assert_eq!(val, "dispatched_value");

    // Use run_cli with Commands::Delete
    let del_args = Cli {
        project: Some(project.to_string()),
        scope: scope.to_string(),
        db: Some(db_path.clone()),
        command: Some(Commands::Delete {
            key: "DISPATCH_KEY".to_string(),
        }),
    };
    run_cli(del_args).expect("run_cli Delete should succeed");

    let val = get_secret_value(&db_path, project, scope, "envault-default-password", "DISPATCH_KEY");
    assert!(val.is_err());
}

#[test]
fn test_cli_binary_e2e_invocations() {
    let bin_path = env!("CARGO_BIN_EXE_envault");
    let temp = tempdir().expect("tempdir should be created");
    let db_path = temp.path().join("vault.db");
    let project = "test-proj-binary-e2e";
    let scope = "testing";

    // 1. envault set KEY=VAL
    let set_output = Command::new(bin_path)
        .env("ENVAULT_PASSWORD", "bin-password-123")
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--project",
            project,
            "--scope",
            scope,
            "set",
            "CONFIG_VAR=my_awesome_config_value",
        ])
        .output()
        .expect("failed to execute set binary");
    assert!(
        set_output.status.success(),
        "set command failed: {}",
        String::from_utf8_lossy(&set_output.stderr)
    );

    // 2. envault get KEY
    let get_output = Command::new(bin_path)
        .env("ENVAULT_PASSWORD", "bin-password-123")
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--project",
            project,
            "--scope",
            scope,
            "get",
            "CONFIG_VAR",
        ])
        .output()
        .expect("failed to execute get binary");
    assert!(
        get_output.status.success(),
        "get command failed: {}",
        String::from_utf8_lossy(&get_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&get_output.stdout).trim(),
        "my_awesome_config_value"
    );

    // 3. envault list
    let list_output = Command::new(bin_path)
        .env("ENVAULT_PASSWORD", "bin-password-123")
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--project",
            project,
            "--scope",
            scope,
            "list",
        ])
        .output()
        .expect("failed to execute list binary");
    assert!(
        list_output.status.success(),
        "list command failed: {}",
        String::from_utf8_lossy(&list_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&list_output.stdout).trim(),
        "CONFIG_VAR"
    );

    // 4. envault delete KEY
    let delete_output = Command::new(bin_path)
        .env("ENVAULT_PASSWORD", "bin-password-123")
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--project",
            project,
            "--scope",
            scope,
            "delete",
            "CONFIG_VAR",
        ])
        .output()
        .expect("failed to execute delete binary");
    assert!(
        delete_output.status.success(),
        "delete command failed: {}",
        String::from_utf8_lossy(&delete_output.stderr)
    );

    // 5. envault get KEY after delete should fail
    let get_after_delete = Command::new(bin_path)
        .env("ENVAULT_PASSWORD", "bin-password-123")
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--project",
            project,
            "--scope",
            scope,
            "get",
            "CONFIG_VAR",
        ])
        .output()
        .expect("failed to execute get binary");
    assert!(!get_after_delete.status.success());
}
