use mcpipe::scanner::workspace::WorkspaceScanner;
use mcpipe::discovery::{BackendKind, SourceScanner};
use tempfile::TempDir;
use std::fs;

#[tokio::test]
async fn finds_openapi_yaml_in_workspace() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path().join("myrepo");
    fs::create_dir_all(&repo).unwrap();
    fs::write(repo.join("openapi.yaml"), r#"
openapi: "3.0.0"
info:
  title: Test API
  version: "1.0"
paths: {}
"#).unwrap();

    let scanner = WorkspaceScanner::from_roots(vec![dir.path().to_string_lossy().to_string()]);
    let sources = scanner.scan().await;

    assert_eq!(sources.len(), 1);
    assert!(matches!(sources[0].kind, BackendKind::OpenApiFile { .. }));
    assert!(sources[0].name.contains("myrepo"));
}

#[tokio::test]
async fn ignores_non_openapi_yaml() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path().join("myrepo");
    fs::create_dir_all(&repo).unwrap();
    fs::write(repo.join("random.yaml"), "key: value\n").unwrap();

    let scanner = WorkspaceScanner::from_roots(vec![dir.path().to_string_lossy().to_string()]);
    let sources = scanner.scan().await;
    assert!(sources.is_empty());
}

#[tokio::test]
async fn skips_target_and_node_modules() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("myrepo/target/debug");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("openapi.yaml"), "openapi: \"3.0.0\"\npaths: {}\n").unwrap();

    let node = dir.path().join("myrepo/node_modules/pkg");
    fs::create_dir_all(&node).unwrap();
    fs::write(node.join("openapi.json"), r#"{"openapi":"3.0.0","paths":{}}"#).unwrap();

    let scanner = WorkspaceScanner::from_roots(vec![dir.path().to_string_lossy().to_string()]);
    let sources = scanner.scan().await;
    assert!(sources.is_empty(), "should skip target/ and node_modules/");
}
