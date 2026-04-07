use async_trait::async_trait;
use serde::Deserialize;

use crate::backend::Backend;
use crate::domain::{ArgMap, BackendError, CommandDef, ParamDef, ParamLocation};

// ── manifest types ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CliManifest {
    commands: Vec<ManifestCommand>,
}

#[derive(Debug, Deserialize)]
struct ManifestCommand {
    name: String,
    description: String,
    params: Vec<ManifestParam>,
}

#[derive(Debug, Deserialize)]
struct ManifestParam {
    name: String,
    flag: String,
    required: bool,
    description: String,
    #[serde(rename = "type")]
    ty: String,
}

// ── CliBackend ────────────────────────────────────────────────────────────────

pub struct CliBackend {
    command: String,
}

impl CliBackend {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
        }
    }
}

#[async_trait]
impl Backend for CliBackend {
    async fn discover(&self) -> Result<Vec<CommandDef>, BackendError> {
        use tokio::process::Command;

        let output = Command::new(&self.command)
            .args(["schema", "--json"])
            .output()
            .await
            .map_err(|e| {
                BackendError::Discovery(format!("failed to run `{}`: {}", self.command, e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BackendError::Discovery(format!(
                "`{} schema` exited non-zero: {}",
                self.command, stderr
            )));
        }

        let manifest: CliManifest = serde_json::from_slice(&output.stdout)
            .map_err(|e| BackendError::Schema(format!("invalid schema JSON: {e}")))?;

        let cmds = manifest
            .commands
            .into_iter()
            .map(|mc| {
                let name = mc.name.replace(' ', "-");
                let params = mc
                    .params
                    .into_iter()
                    .map(|p| ParamDef {
                        name: p.name.clone(),
                        original_name: p.flag.trim_start_matches('-').to_string(),
                        required: p.required,
                        description: p.description,
                        location: ParamLocation::ToolInput,
                        schema: type_str_to_schema(&p.ty),
                    })
                    .collect();
                CommandDef {
                    name,
                    description: mc.description,
                    params,
                    source_name: self.command.clone(),
                }
            })
            .collect();

        Ok(cmds)
    }

    async fn execute(
        &self,
        cmd: &CommandDef,
        args: ArgMap,
    ) -> Result<serde_json::Value, BackendError> {
        use tokio::process::Command;

        // "todo-list" -> ["todo", "list"]; single-word "search" -> ["search"].
        // Constraint: command names with >1 hyphen (e.g. "todo-add-tag") are not supported —
        // the manifest must use at most one level of nesting.
        let parts: Vec<&str> = cmd.name.splitn(2, '-').collect();
        let mut argv: Vec<String> = parts.iter().map(|s| s.to_string()).collect();
        argv.push("--json".to_string());

        for param in &cmd.params {
            let key = &param.name;
            if let Some(val) = args.get(key) {
                let flag = format!("--{}", param.original_name.replace('_', "-"));
                match val {
                    serde_json::Value::Array(items) => {
                        for item in items {
                            let s = match item {
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            argv.push(flag.clone());
                            argv.push(s);
                        }
                    }
                    serde_json::Value::Null => {}
                    other => {
                        argv.push(flag);
                        argv.push(match other {
                            serde_json::Value::String(s) => s.clone(),
                            n => n.to_string(),
                        });
                    }
                }
            }
        }

        let output = Command::new(&self.command)
            .args(&argv)
            .output()
            .await
            .map_err(|e| BackendError::Transport(format!("spawn error: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BackendError::Execution(format!(
                "`{} {}` failed: {}",
                self.command,
                argv.join(" "),
                stderr
            )));
        }

        let value: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| BackendError::Execution(format!("JSON parse error: {e}")))?;

        Ok(value)
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn type_str_to_schema(ty: &str) -> serde_json::Value {
    match ty {
        "integer" => serde_json::json!({"type": "integer"}),
        "boolean" => serde_json::json!({"type": "boolean"}),
        "array" => serde_json::json!({"type": "array", "items": {"type": "string"}}),
        _ => serde_json::json!({"type": "string"}),
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "integration")]
    #[tokio::test]
    async fn discover_doob_commands() {
        let backend = CliBackend::new("doob");
        let cmds = backend.discover().await.unwrap();
        assert!(cmds.len() > 5);
        assert!(cmds.iter().any(|c| c.name == "todo-list"));
        assert!(cmds.iter().any(|c| c.name == "todo-add"));
        assert!(cmds.iter().any(|c| c.name == "search"));
    }

    #[cfg(feature = "integration")]
    #[tokio::test]
    async fn execute_doob_todo_list() {
        let backend = CliBackend::new("doob");
        let cmds = backend.discover().await.unwrap();
        let list_cmd = cmds.iter().find(|c| c.name == "todo-list").unwrap().clone();
        let result = backend
            .execute(&list_cmd, std::collections::HashMap::new())
            .await
            .unwrap();
        assert!(result.get("todos").is_some() || result.get("count").is_some());
    }
}
