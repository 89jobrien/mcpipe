use std::process::Command;

#[test]
fn tool_argument_named_completions_is_not_intercepted() {
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
    assert!(!stdout.contains("module completions"));
    assert!(stdout.contains("Usage: mcpipe show-pet-by-id"));
}
