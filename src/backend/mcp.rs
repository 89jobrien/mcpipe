use async_trait::async_trait;
use eventsource_client::{Client as SseClient, ClientBuilder as SseClientBuilder, SSE};
use futures::StreamExt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, oneshot};

use crate::domain::{ArgMap, BackendError, CommandDef, ParamDef, ParamLocation};
use crate::backend::openapi::to_kebab;
use super::Backend;

pub struct McpBackend {
    inner: McpTransport,
}

#[allow(dead_code)]
enum McpTransport {
    Stdio { command: String },
    Http  { url: String, headers: Vec<(String, String)> },
}

impl McpBackend {
    pub fn from_stdio(command: String) -> Self {
        Self { inner: McpTransport::Stdio { command } }
    }

    pub fn from_http(url: String, headers: Vec<(String, String)>) -> Self {
        Self { inner: McpTransport::Http { url, headers } }
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

    async fn run_http_session<F, Fut>(&self, url: &str, headers: &[(String, String)], f: F) -> Result<serde_json::Value, BackendError>
    where
        F: FnOnce(HttpSession) -> Fut,
        Fut: std::future::Future<Output = Result<serde_json::Value, BackendError>>,
    {
        // Parse base URL to reconstruct absolute POST URL from the relative endpoint path
        let base_url = {
            let parsed = url::Url::parse(url)
                .map_err(|e| BackendError::Transport(format!("invalid URL: {e}")))?;
            format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or("localhost"))
                + &parsed.port().map(|p| format!(":{p}")).unwrap_or_default()
        };

        // Build SSE client
        let mut builder = SseClientBuilder::for_url(url)
            .map_err(|e| BackendError::Transport(format!("SSE client build: {e}")))?;
        for (name, value) in headers {
            builder = builder.header(name, value)
                .map_err(|e| BackendError::Transport(format!("SSE header: {e}")))?;
        }
        let sse_client = builder.build();
        let mut stream = Box::pin(sse_client.stream());

        // Wait for the endpoint event
        let post_path = loop {
            match stream.next().await {
                Some(Ok(SSE::Event(evt))) if evt.event_type == "endpoint" => {
                    break evt.data;
                }
                Some(Ok(_)) => continue,
                Some(Err(e)) => return Err(BackendError::Transport(format!("SSE error waiting for endpoint: {e}"))),
                None => return Err(BackendError::Transport("SSE stream closed before endpoint event".to_string())),
            }
        };

        let post_url = if post_path.starts_with("http://") || post_path.starts_with("https://") {
            post_path
        } else {
            format!("{base_url}{post_path}")
        };

        let session = HttpSession::new(post_url, headers.to_vec(), stream);
        session.send_initialize().await?;
        f(session).await
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

/// SSE stream type alias
type SseStream = std::pin::Pin<Box<dyn futures::Stream<Item = Result<SSE, eventsource_client::Error>> + Send + Sync>>;

struct HttpSession {
    post_url: String,
    headers: Vec<(String, String)>,
    http_client: reqwest::Client,
    /// Pending response channels keyed by request id
    pending: std::sync::Arc<Mutex<std::collections::HashMap<u64, oneshot::Sender<serde_json::Value>>>>,
    next_id: Mutex<u64>,
    /// Abort handle for the background SSE stream task; aborted on drop.
    _stream_abort: tokio::task::AbortHandle,
}

impl HttpSession {
    fn new(post_url: String, headers: Vec<(String, String)>, stream: SseStream) -> Self {
        let pending: std::sync::Arc<Mutex<std::collections::HashMap<u64, oneshot::Sender<serde_json::Value>>>> =
            std::sync::Arc::new(Mutex::new(std::collections::HashMap::new()));
        let pending_clone = pending.clone();

        // Drive the SSE stream in background, routing responses to waiting senders.
        // The AbortHandle is stored on HttpSession and aborted on drop; we drop the
        // JoinHandle here so the task runs detached (abort_handle remains valid).
        let stream_task = tokio::spawn(async move {
            let mut stream = stream;
            while let Some(item) = stream.next().await {
                match item {
                    Ok(SSE::Event(evt)) if evt.event_type == "message" => {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&evt.data) {
                            let Some(id) = val.get("id").and_then(|v| v.as_u64()) else { continue };
                            if let Some(tx) = pending_clone.lock().await.remove(&id) {
                                let _ = tx.send(val);
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });

        Self {
            post_url,
            headers,
            http_client: reqwest::Client::new(),
            pending,
            next_id: Mutex::new(1),
            _stream_abort: stream_task.abort_handle(),
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

        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let mut req = self.http_client.post(&self.post_url)
            .header("Content-Type", "application/json")
            .json(&msg);
        for (name, value) in &self.headers {
            req = req.header(name.as_str(), value.as_str());
        }

        req.send().await
            .map_err(|e| BackendError::Transport(format!("POST {}: {e}", self.post_url)))?;

        // Wait for the SSE stream to deliver the response
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            rx,
        ).await
            .map_err(|_| BackendError::Transport("timeout waiting for MCP response".to_string()))?
            .map_err(|_| BackendError::Transport("response channel dropped".to_string()))?;

        if let Some(err) = response.get("error") {
            return Err(BackendError::Execution(err.to_string()));
        }
        Ok(response.get("result").cloned().unwrap_or(serde_json::Value::Null))
    }

    async fn send_notification(&self, method: &str, params: serde_json::Value) -> Result<(), BackendError> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let mut req = self.http_client.post(&self.post_url)
            .header("Content-Type", "application/json")
            .json(&msg);
        for (name, value) in &self.headers {
            req = req.header(name.as_str(), value.as_str());
        }
        req.send().await
            .map_err(|e| BackendError::Transport(format!("POST notification: {e}")))?;
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

impl Drop for HttpSession {
    fn drop(&mut self) {
        self._stream_abort.abort();
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
            McpTransport::Http { url, headers } => {
                let url = url.clone();
                let headers = headers.clone();
                self.run_http_session(&url, &headers, |session| async move {
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
            McpTransport::Http { url, headers } => {
                let url = url.clone();
                let headers = headers.clone();
                let tool_name = cmd.source_name.clone();
                self.run_http_session(&url, &headers, |session| async move {
                    let result = session.send_request("tools/call", serde_json::json!({
                        "name": tool_name,
                        "arguments": args,
                    })).await?;
                    Ok(result.get("content").cloned().unwrap_or(result))
                }).await
            }
        }
    }
}
