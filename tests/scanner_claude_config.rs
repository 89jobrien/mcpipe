use mcpipe::discovery::{BackendKind, DiscoveredSource};

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
