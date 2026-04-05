use async_trait::async_trait;
use std::collections::HashSet;

use crate::discovery::{BackendKind, DiscoveredSource, SourceScanner};

/// Scans Claude Code config files and project `.mcp.json` files for MCP server definitions.
pub struct ClaudeConfigScanner {
    settings_paths: Vec<String>,
    mcp_paths: Vec<String>,
}

impl ClaudeConfigScanner {
    pub fn from_paths(settings_paths: Vec<String>, mcp_paths: Vec<String>) -> Self {
        Self { settings_paths, mcp_paths }
    }

    pub fn default_env() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let claude_settings = home.join(".claude/settings.json");

        let mut mcp_paths = vec![];
        if let Ok(entries) = std::fs::read_dir(home.join("dev")) {
            for entry in entries.flatten() {
                let p = entry.path();
                for name in &[".mcp.json", "mcp.json"] {
                    let candidate = p.join(name);
                    if candidate.exists() {
                        mcp_paths.push(candidate.to_string_lossy().to_string());
                    }
                }
                let cursor = p.join(".cursor/mcp.json");
                if cursor.exists() {
                    mcp_paths.push(cursor.to_string_lossy().to_string());
                }
            }
        }

        Self {
            settings_paths: if claude_settings.exists() {
                vec![claude_settings.to_string_lossy().to_string()]
            } else {
                vec![]
            },
            mcp_paths,
        }
    }

    fn parse_server_entry(
        name: &str,
        entry: &serde_json::Value,
        origin: &str,
        seen: &mut HashSet<String>,
    ) -> Option<DiscoveredSource> {
        if let Some(url) = entry.get("url").and_then(|v| v.as_str()) {
            let dedup_key = format!("http:{url}");
            if !seen.insert(dedup_key) {
                return None;
            }
            return Some(DiscoveredSource {
                name: name.to_string(),
                kind: BackendKind::McpHttp { url: url.to_string() },
                origin: origin.to_string(),
            });
        }

        let cmd = entry.get("command").and_then(|v| v.as_str())?;
        let args: Vec<String> = entry
            .get("args")
            .and_then(|a| a.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(String::from).collect())
            .unwrap_or_default();
        let command_str = if args.is_empty() {
            cmd.to_string()
        } else {
            format!("{} {}", cmd, args.join(" "))
        };

        // Dedup by (binary_basename, args) so that "/usr/bin/foo bar" and "foo bar"
        // are treated as the same server regardless of absolute vs relative path.
        let binary_basename = std::path::Path::new(cmd)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(cmd);
        let dedup_key = if args.is_empty() {
            format!("stdio:{binary_basename}")
        } else {
            format!("stdio:{binary_basename} {}", args.join(" "))
        };
        if !seen.insert(dedup_key) {
            return None;
        }

        Some(DiscoveredSource {
            name: name.to_string(),
            kind: BackendKind::McpStdio { command: command_str },
            origin: origin.to_string(),
        })
    }

    fn parse_file(path: &str, seen: &mut HashSet<String>) -> Vec<DiscoveredSource> {
        let Ok(text) = std::fs::read_to_string(path) else { return vec![] };
        let Ok(json): Result<serde_json::Value, _> = serde_json::from_str(&text) else {
            return vec![];
        };

        let servers = json.get("mcpServers")
            .and_then(|v| v.as_object())
            .or_else(|| json.as_object());

        let Some(servers) = servers else { return vec![] };

        servers
            .iter()
            .filter_map(|(name, entry)| {
                if !entry.is_object() { return None; }
                Self::parse_server_entry(name, entry, path, seen)
            })
            .collect()
    }
}

#[async_trait]
impl SourceScanner for ClaudeConfigScanner {
    async fn scan(&self) -> Vec<DiscoveredSource> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut sources = vec![];

        for path in &self.settings_paths {
            sources.extend(Self::parse_file(path, &mut seen));
        }
        for path in &self.mcp_paths {
            sources.extend(Self::parse_file(path, &mut seen));
        }

        sources
    }
}
