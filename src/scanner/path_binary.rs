use async_trait::async_trait;

use crate::discovery::{BackendKind, DiscoveredSource, SourceScanner};

/// Well-known MCP stdio binaries. Each entry is a binary name that, when found
/// on PATH, is auto-registered as a McpStdio backend.
const WELL_KNOWN_BINARIES: &[&str] = &[
    "obfsck-mcp", // obfsck MCP server (obfsck-11) — audit + generate-filters tools
];

/// Scans PATH for well-known MCP stdio binaries and registers each one found.
pub struct PathBinaryScanner {
    path_dirs: Vec<String>,
}

impl PathBinaryScanner {
    /// Use the current process PATH.
    pub fn new() -> Self {
        let path = std::env::var("PATH").unwrap_or_default();
        Self::with_path(&path)
    }

    /// Use a custom PATH string (colon-separated on Unix).
    pub fn with_path(path: &str) -> Self {
        let sep = if cfg!(windows) { ';' } else { ':' };
        Self {
            path_dirs: path.split(sep).map(String::from).collect(),
        }
    }

    /// Returns the list of well-known binary names checked during scan.
    pub fn well_known_names() -> &'static [&'static str] {
        WELL_KNOWN_BINARIES
    }

    fn is_executable(path: &std::path::Path) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::metadata(path)
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        }
        #[cfg(not(unix))]
        {
            path.exists()
        }
    }
}

impl Default for PathBinaryScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SourceScanner for PathBinaryScanner {
    async fn scan(&self) -> Vec<DiscoveredSource> {
        let mut sources = vec![];

        for name in WELL_KNOWN_BINARIES {
            let found = self.path_dirs.iter().any(|dir| {
                let candidate = std::path::Path::new(dir).join(name);
                Self::is_executable(&candidate)
            });

            if found {
                sources.push(DiscoveredSource {
                    name: name.to_string(),
                    kind: BackendKind::McpStdio {
                        command: name.to_string(),
                    },
                    origin: "PATH".to_string(),
                });
            }
        }

        sources
    }
}
