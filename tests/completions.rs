//! Regression tests for global flags and generated subcommand help.

use std::process::Command;

#[test]
fn generated_subcommand_help_succeeds() {
    let spec = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/petstore.json");
    let output = Command::new(env!("CARGO_BIN_EXE_mcpipe"))
        .args([
            "--spec",
            spec,
            "show-pet-by-id",
            "--pet-id",
            "completions",
            "--help",
        ])
        .output()
        .expect("run mcpipe");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(!stdout.contains("module completions"));
    assert!(stdout.contains("Usage: mcpipe show-pet-by-id"));
    assert!(stderr.is_empty());
}

#[test]
fn version_flag_prints_package_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_mcpipe"))
        .arg("--version")
        .output()
        .expect("run mcpipe");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert_eq!(
        stdout.trim(),
        format!("mcpipe {}", env!("CARGO_PKG_VERSION"))
    );
    assert!(stderr.is_empty());
}
