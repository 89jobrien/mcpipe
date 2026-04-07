use mcpipe::backend::Backend;
use mcpipe::backend::mcp::McpBackend;
use mcpipe::domain::ParamLocation;

#[tokio::test]
async fn mcp_stdio_discover() {
    let python = which_python();
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mcp_echo.py");
    let cmd = format!("{python} {script}");

    let backend = McpBackend::from_stdio(cmd);
    let cmds = backend.discover().await.unwrap();

    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].name, "echo");
    assert_eq!(cmds[0].params.len(), 1);
    assert_eq!(cmds[0].params[0].name, "message");
    assert!(cmds[0].params[0].required);
    assert!(matches!(
        cmds[0].params[0].location,
        ParamLocation::ToolInput
    ));
}

#[tokio::test]
async fn mcp_stdio_execute() {
    let python = which_python();
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mcp_echo.py");
    let cmd = format!("{python} {script}");

    let backend = McpBackend::from_stdio(cmd);
    let cmds = backend.discover().await.unwrap();
    let echo_cmd = cmds.iter().find(|c| c.name == "echo").unwrap();

    let mut args = std::collections::HashMap::new();
    args.insert("message".to_string(), serde_json::json!("hello mcpipe"));

    let result = backend.execute(echo_cmd, args).await.unwrap();
    // MCP returns content array: [{"type": "text", "text": "..."}]
    let text = result
        .get(0)
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| result.as_str().unwrap_or(""));
    assert_eq!(text, "hello mcpipe");
}

fn which_python() -> String {
    for candidate in &["python3", "python"] {
        if std::process::Command::new(candidate)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return candidate.to_string();
        }
    }
    panic!("python3 not found — required for MCP adapter tests");
}
