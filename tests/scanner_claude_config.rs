use mcpipe::discovery::{BackendKind, DiscoveredSource};
use mcpipe::scanner::claude_config::ClaudeConfigScanner;
use mcpipe::discovery::SourceScanner;

#[test]
fn discovered_source_roundtrip() {
    let src = DiscoveredSource {
        name: "my-server".to_string(),
        kind: BackendKind::McpStdio { command: "my-server mcp".to_string() },
        origin: "~/.claude/settings.json".to_string(),
    };
    assert_eq!(src.name, "my-server");
    assert!(matches!(src.kind, BackendKind::McpStdio { .. }));
}

#[tokio::test]
async fn scans_settings_json_mcp_servers() {
    let settings_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/claude_settings.json"
    );
    let scanner = ClaudeConfigScanner::from_paths(
        vec![settings_path.to_string()],
        vec![],
    );
    let sources = scanner.scan().await;
    assert_eq!(sources.len(), 2);
    let names: Vec<_> = sources.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"devloop"));
    assert!(names.contains(&"personal-mcp"));
    for src in &sources {
        assert!(matches!(src.kind, BackendKind::McpStdio { .. }));
        assert_eq!(src.origin, settings_path);
    }
}

#[tokio::test]
async fn scans_project_mcp_json() {
    let mcp_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/mcp_project.json"
    );
    let scanner = ClaudeConfigScanner::from_paths(
        vec![],
        vec![mcp_path.to_string()],
    );
    let sources = scanner.scan().await;
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].name, "devloop-pipeline");
    assert!(matches!(sources[0].kind, BackendKind::McpStdio { .. }));
}

#[tokio::test]
async fn deduplicates_same_command() {
    let settings_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/claude_settings.json"
    );
    let mcp_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/mcp_project.json"
    );
    let scanner = ClaudeConfigScanner::from_paths(
        vec![settings_path.to_string()],
        vec![mcp_path.to_string()],
    );
    let sources = scanner.scan().await;
    // settings has devloop + personal-mcp; project has devloop-pipeline (different command)
    assert_eq!(sources.len(), 3);
}
