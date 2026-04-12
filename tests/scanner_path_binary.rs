/// Tests for PathBinaryScanner — discovers well-known MCP stdio binaries on PATH.
/// Covers mcpipe-21: auto-map obfsck MCP server via --scan.
use mcpipe::discovery::{BackendKind, SourceScanner};
use mcpipe::scanner::path_binary::PathBinaryScanner;

#[tokio::test]
async fn scanner_returns_empty_when_no_binaries_on_path() {
    // Use a PATH that contains nothing useful.
    let scanner = PathBinaryScanner::with_path("/nonexistent/path");
    let sources = scanner.scan().await;
    assert!(sources.is_empty());
}

#[tokio::test]
async fn scanner_discovers_binary_on_path() {
    // Temporarily point PATH at a dir with a fake obfsck-mcp binary.
    let dir = tempfile::tempdir().expect("tempdir");
    let bin_path = dir.path().join("obfsck-mcp");

    // Write a minimal shell script that exits 0 — just needs to be executable.
    std::fs::write(
        &bin_path,
        "#!/bin/sh\nexit 0\n",
    )
    .expect("write fake binary");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&bin_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin_path, perms).unwrap();
    }

    let scanner = PathBinaryScanner::with_path(dir.path().to_str().expect("path str"));
    let sources = scanner.scan().await;

    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].name, "obfsck-mcp");
    assert!(
        matches!(&sources[0].kind, BackendKind::McpStdio { command } if command == "obfsck-mcp"),
        "expected McpStdio with command obfsck-mcp, got {:?}",
        sources[0].kind
    );
    assert_eq!(sources[0].origin, "PATH");
}

#[tokio::test]
async fn scanner_ignores_non_registered_binaries_on_path() {
    let dir = tempfile::tempdir().expect("tempdir");

    // Write a binary that is NOT in the well-known list.
    let bin_path = dir.path().join("some-random-tool");
    std::fs::write(&bin_path, "#!/bin/sh\nexit 0\n").expect("write");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&bin_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin_path, perms).unwrap();
    }

    let scanner = PathBinaryScanner::with_path(dir.path().to_str().expect("path str"));
    let sources = scanner.scan().await;
    assert!(sources.is_empty(), "should not discover unknown binaries");
}

#[tokio::test]
async fn default_scanner_includes_obfsck_mcp_when_on_path() {
    // Verify that the default well-known list includes obfsck-mcp.
    // We don't need it to be on the real PATH — just check the list is configured.
    let registered = PathBinaryScanner::well_known_names();
    assert!(
        registered.contains(&"obfsck-mcp"),
        "obfsck-mcp must be in the well-known list"
    );
}
