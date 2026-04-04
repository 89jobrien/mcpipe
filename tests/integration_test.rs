#![cfg(feature = "integration")]

//! Integration tests — run with `cargo test --features integration`.
//!
//! These tests are excluded from the normal `cargo test` run so they don't
//! block CI without the necessary external fixtures or environment.

use mcpipe::backend::openapi::OpenApiBackend;
use mcpipe::backend::Backend;
use mcpipe::domain::ParamLocation;

/// Smoke-test: discover commands from the bundled petstore fixture.
///
/// This is a fast, self-contained integration test that exercises the full
/// OpenApiBackend pipeline (parse → normalise → discover) without any network
/// calls or external processes.
#[tokio::test]
async fn integration_openapi_discover_petstore() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/petstore.json");
    let backend = OpenApiBackend::from_file(path).expect("failed to load petstore fixture");
    let cmds = backend.discover().await.expect("discover failed");

    assert_eq!(cmds.len(), 3, "expected exactly 3 petstore commands");

    let list = cmds.iter().find(|c| c.name == "list-pets").expect("list-pets missing");
    assert_eq!(list.source_name, "listPets");
    assert!(
        list.params.iter().all(|p| matches!(p.location, ParamLocation::Query | ParamLocation::Path | ParamLocation::Header | ParamLocation::Body)),
        "unexpected param location on list-pets"
    );

    let create = cmds.iter().find(|c| c.name == "create-pet").expect("create-pet missing");
    let name_param = create.params.iter().find(|p| p.name == "name").expect("name param missing");
    assert!(name_param.required, "name param should be required");
    assert!(
        matches!(name_param.location, ParamLocation::Body),
        "name param should be in request body"
    );

    let show = cmds.iter().find(|c| c.name == "show-pet-by-id").expect("show-pet-by-id missing");
    let id_param = show.params.iter().find(|p| p.name == "pet-id").expect("petId param missing");
    assert!(id_param.required, "pet-id param should be required");
    assert!(
        matches!(id_param.location, ParamLocation::Path),
        "pet-id param should be a path parameter"
    );
}

/// Negative test: loading a backend from a non-existent file path should
/// return an error rather than panic.
#[tokio::test]
async fn integration_openapi_missing_file_returns_error() {
    let result = OpenApiBackend::from_file("/nonexistent/path/spec.json");
    assert!(result.is_err(), "expected an error for missing spec file");
}

/// MCP stdio integration: discover tools from the bundled echo fixture.
///
/// Requires Python 3 on PATH.  Skipped automatically when `python3` is absent
/// (the `which_python` helper panics with a clear message in that case).
#[tokio::test]
async fn integration_mcp_stdio_echo() {
    let python = which_python();
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mcp_echo.py");
    let cmd = format!("{python} {script}");

    let backend = mcpipe::backend::mcp::McpBackend::from_stdio(cmd);
    let cmds = backend.discover().await.expect("MCP stdio discover failed");

    assert_eq!(cmds.len(), 1, "echo fixture should expose exactly one tool");
    assert_eq!(cmds[0].name, "echo");
    assert_eq!(cmds[0].params.len(), 1);
    assert_eq!(cmds[0].params[0].name, "message");
    assert!(cmds[0].params[0].required);

    // Execute the tool and verify round-trip.
    let mut args = std::collections::HashMap::new();
    args.insert("message".to_string(), serde_json::json!("integration test"));

    let result = backend
        .execute(&cmds[0], args)
        .await
        .expect("MCP stdio execute failed");

    let text = result
        .get(0)
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| result.as_str().unwrap_or(""));
    assert_eq!(text, "integration test");
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
    panic!("python3 not found — required for MCP integration tests");
}
