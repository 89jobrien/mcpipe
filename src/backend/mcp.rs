use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::domain::{ArgMap, BackendError, CommandDef, ParamDef, ParamLocation};
use crate::backend::openapi::to_kebab;
use super::Backend;

pub struct McpBackend {
    inner: McpTransport,
}

#[allow(dead_code)]
enum McpTransport {
    Stdio { command: String },
    Http  { url: String, auth_headers: Vec<(String, String)> },
}

impl McpBackend {
    pub fn from_stdio(command: String) -> Self {
        Self { inner: McpTransport::Stdio { command } }
    }

    pub fn from_http(url: String, auth_headers: Vec<(String, String)>) -> Self {
        Self { inner: McpTransport::Http { url, auth_headers } }
    }

    async fn run_stdio_session<F, Fut>(&self, command: &str, f: F) -> Result<serde_json::Value, BackendError>
    where
        F: FnOnce(StdioSession) -> Fut,
        Fut: std::future::Future<Output = Result<serde_json::Value, BackendError>>,
    {
        let mut parts = command.split_whitespace();
        let prog = parts.next().ok_or_else(|| BackendError::Transport("empty command".to_string()))?;
        let args: Vec<&str> = parts.collect();

        let mut child = Command::new(prog)
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| BackendError::Transport(format!("spawn {command}: {e}")))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| BackendError::Transport("no stdin".to_string()))?;
        let stdout = child.stdout.take()
            .ok_or_else(|| BackendError::Transport("no stdout".to_string()))?;

        let session = StdioSession::new(stdin, stdout);
        session.send_initialize().await?;

        let result = f(session).await;
        let _ = child.kill().await;
        result
    }
}

struct StdioSession {
    stdin: Mutex<tokio::process::ChildStdin>,
    reader: Mutex<BufReader<tokio::process::ChildStdout>>,
    next_id: Mutex<u64>,
}

impl StdioSession {
    fn new(stdin: tokio::process::ChildStdin, stdout: tokio::process::ChildStdout) -> Self {
        Self {
            stdin: Mutex::new(stdin),
            reader: Mutex::new(BufReader::new(stdout)),
            next_id: Mutex::new(1),
        }
    }

    async fn next_id(&self) -> u64 {
        let mut id = self.next_id.lock().await;
        let cur = *id;
        *id += 1;
        cur
    }

    async fn send_request(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, BackendError> {
        let id = self.next_id().await;
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let line = format!("{}\n", serde_json::to_string(&msg).expect("BUG: json literal failed to serialize"));

        self.stdin.lock().await
            .write_all(line.as_bytes()).await
            .map_err(|e| BackendError::Transport(format!("write: {e}")))?;

        let mut reader = self.reader.lock().await;
        loop {
            let mut buf = String::new();
            reader.read_line(&mut buf).await
                .map_err(|e| BackendError::Transport(format!("read: {e}")))?;
            if buf.is_empty() {
                return Err(BackendError::Transport("EOF from MCP server".to_string()));
            }
            let val: serde_json::Value = match serde_json::from_str(buf.trim()) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if val.get("id").and_then(|v| v.as_u64()) == Some(id) {
                if let Some(err) = val.get("error") {
                    return Err(BackendError::Execution(err.to_string()));
                }
                return Ok(val.get("result").cloned().unwrap_or(serde_json::Value::Null));
            }
        }
    }

    async fn send_notification(&self, method: &str, params: serde_json::Value) -> Result<(), BackendError> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let line = format!("{}\n", serde_json::to_string(&msg).expect("BUG: json literal failed to serialize"));
        self.stdin.lock().await
            .write_all(line.as_bytes()).await
            .map_err(|e| BackendError::Transport(format!("write notification: {e}")))?;
        Ok(())
    }

    async fn send_initialize(&self) -> Result<(), BackendError> {
        self.send_request("initialize", serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "mcpipe", "version": "0.1.0" }
        })).await?;
        self.send_notification("notifications/initialized", serde_json::json!({})).await?;
        Ok(())
    }
}

fn tools_to_commands(tools: &[serde_json::Value]) -> Vec<CommandDef> {
    tools.iter().map(|tool| {
        let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
        let description = tool.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let schema = tool.get("inputSchema").cloned().unwrap_or(serde_json::json!({}));

        let required_fields: Vec<&str> = schema.get("required")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        let mut params = vec![];
        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            for (pname, pschema) in props {
                let required = required_fields.contains(&pname.as_str());
                let desc = pschema.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                params.push(ParamDef {
                    name: to_kebab(pname),
                    original_name: pname.clone(),
                    required,
                    description: desc,
                    location: ParamLocation::ToolInput,
                    schema: pschema.clone(),
                });
            }
        }

        CommandDef {
            name: to_kebab(name),
            description,
            source_name: name.to_string(),
            params,
        }
    }).collect()
}

#[async_trait]
impl Backend for McpBackend {
    async fn discover(&self) -> Result<Vec<CommandDef>, BackendError> {
        match &self.inner {
            McpTransport::Stdio { command } => {
                let command = command.clone();
                self.run_stdio_session(&command, |session| async move {
                    let result = session.send_request("tools/list", serde_json::json!({})).await?;
                    let tools = result.get("tools")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let cmds = tools_to_commands(&tools);
                    serde_json::to_value(cmds)
                        .map_err(|e| BackendError::Schema(e.to_string()))
                }).await
                .and_then(|v| serde_json::from_value(v).map_err(|e| BackendError::Schema(e.to_string())))
            }
            McpTransport::Http { .. } => {
                Err(BackendError::Transport("MCP HTTP transport not yet implemented".to_string()))
            }
        }
    }

    async fn execute(&self, cmd: &CommandDef, args: ArgMap) -> Result<serde_json::Value, BackendError> {
        match &self.inner {
            McpTransport::Stdio { command } => {
                let command = command.clone();
                let tool_name = cmd.source_name.clone();
                self.run_stdio_session(&command, |session| async move {
                    let result = session.send_request("tools/call", serde_json::json!({
                        "name": tool_name,
                        "arguments": args,
                    })).await?;
                    Ok(result.get("content").cloned().unwrap_or(result))
                }).await
            }
            McpTransport::Http { .. } => {
                Err(BackendError::Transport("MCP HTTP transport not yet implemented".to_string()))
            }
        }
    }
}
