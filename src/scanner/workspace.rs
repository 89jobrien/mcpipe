use async_trait::async_trait;
use std::path::Path;

use crate::discovery::{BackendKind, DiscoveredSource, SourceScanner};

const SKIP_DIRS: &[&str] = &[
    "target",
    "node_modules",
    ".git",
    ".cache",
    "dist",
    "build",
    "baml_client",
    "vendor",
    "generated",
    "gen",
    ".venv",
    "venv",
];
const OPENAPI_FILENAMES: &[&str] = &[
    "openapi.yaml",
    "openapi.yml",
    "openapi.json",
    "swagger.yaml",
    "swagger.yml",
    "swagger.json",
];

/// Walks workspace root directories looking for OpenAPI spec files.
/// Skips build artifact directories (target/, node_modules/, etc.).
pub struct WorkspaceScanner {
    roots: Vec<String>,
}

impl WorkspaceScanner {
    pub fn from_roots(roots: Vec<String>) -> Self {
        Self { roots }
    }

    /// Default: scan ~/dev
    pub fn default_env() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        Self::from_roots(vec![home.join("dev").to_string_lossy().to_string()])
    }

    fn is_openapi(path: &Path) -> bool {
        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !OPENAPI_FILENAMES.contains(&fname) {
            return false;
        }
        let Ok(bytes) = std::fs::read(path) else {
            return false;
        };
        let snippet = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
        snippet.contains("openapi:") || snippet.contains(r#""openapi":"#)
    }

    fn walk(root: &Path, repo_name: &str) -> Vec<DiscoveredSource> {
        let mut sources = vec![];
        let Ok(entries) = std::fs::read_dir(root) else {
            return sources;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if SKIP_DIRS.contains(&name) {
                    continue;
                }
                sources.extend(Self::walk(&path, repo_name));
            } else if path.is_file() && Self::is_openapi(&path) {
                let display = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("openapi");
                let name = format!("{repo_name}/{display}");

                sources.push(DiscoveredSource {
                    name,
                    kind: BackendKind::OpenApiFile {
                        path: path.to_string_lossy().to_string(),
                    },
                    origin: root.to_string_lossy().to_string(),
                });
            }
        }

        sources
    }
}

#[async_trait]
impl SourceScanner for WorkspaceScanner {
    async fn scan(&self) -> Vec<DiscoveredSource> {
        let mut sources = vec![];
        for root in &self.roots {
            let Ok(entries) = std::fs::read_dir(root) else {
                continue;
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let dir_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if SKIP_DIRS.contains(&dir_name) {
                        continue;
                    }
                    let repo_name = dir_name.to_string();
                    sources.extend(Self::walk(&p, &repo_name));
                }
            }
        }
        sources
    }
}
