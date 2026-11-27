//! End-to-end check for the master password prompt.
//!
//! The prompt must hide every typed character. rpassword writes the mask only
//! when the input is a real terminal, so this test runs the binary inside a
//! pseudo terminal. On systems without a usable `script` binary the test does
//! nothing and passes.

use std::process::{Command, Stdio};
use std::time::Duration;

use tempfile::tempdir;

/// Returns true when the `script` utility can allocate a pseudo terminal.
///
/// The flag set differs between macOS and Linux, so the probe runs a real
/// command through `script` instead of asking for a version string.
fn script_available() -> bool {
    let temp = match tempdir() {
        Ok(dir) => dir,
        Err(_) => return false,
    };
    Command::new("script")
        .arg("-q")
        .arg(temp.path().join("probe.txt"))
        .arg("true")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[test]
fn test_password_prompt_masks_typed_characters() {
    if !script_available() {
        eprintln!("skipping: the script utility is not available");
        return;
    }

    let bin_path = env!("CARGO_BIN_EXE_envault");
    let temp = tempdir().expect("tempdir should be created");
    let db_path = temp.path().join("vault.db");
    let typescript = temp.path().join("typescript.txt");
    let password = "mask-test-secret-pw";

    // Store a secret first so the later read has a valid vault and key.
    let seed = Command::new(bin_path)
        .env("ENVAULT_PASSWORD", password)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--project",
            "mask-proj",
            "--scope",
            "default",
            "set",
            "MASK_KEY=mask_value",
        ])
        .output()
        .expect("seed set command should run");
    assert!(seed.status.success(), "seed set command failed");

    // Run `get` without ENVAULT_PASSWORD so the binary prompts for the password.
    // The child inherits the pseudo terminal from `script`, therefore the prompt
    // takes the interactive path.
    let mut child = Command::new("script")
        .arg("-q")
        .arg(&typescript)
        .arg(bin_path)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--project",
            "mask-proj",
            "--scope",
            "default",
            "get",
            "MASK_KEY",
        ])
        .env_remove("ENVAULT_PASSWORD")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("script wrapper should start");

    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().expect("child stdin should be open");
        // Wait for the prompt to appear before typing.
        std::thread::sleep(Duration::from_millis(600));
        writeln!(stdin, "{password}").expect("password should be written to the pseudo terminal");
        stdin.flush().expect("flush should succeed");
    }

    let _ = child.wait().expect("child should finish");

    let transcript = std::fs::read_to_string(&typescript)
        .expect("script should write a transcript file")
        .replace('\r', "");

    assert!(
        transcript.contains("Enter master password:"),
        "prompt text is missing from the transcript: {transcript:?}"
    );
    assert!(
        !transcript.contains(password),
        "the plaintext password leaked to the terminal: {transcript:?}"
    );
    assert!(
        transcript.contains(&"*".repeat(password.len())),
        "expected one mask character per typed character: {transcript:?}"
    );
}
