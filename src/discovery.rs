use async_trait::async_trait;

/// One discovered API source — enough to construct a Backend and run discover().
#[derive(Debug, Clone)]
pub struct DiscoveredSource {
    /// Human-readable name (from config key or derived from URL/command).
    pub name: String,
    /// How to connect.
    pub kind: BackendKind,
    /// Where this source was found (config file path, workspace path, etc.).
    pub origin: String,
}

#[derive(Debug, Clone)]
pub enum BackendKind {
    /// MCP over stdio — spawn a process.
    McpStdio { command: String },
    /// MCP over HTTP/SSE.
    McpHttp { url: String },
    /// OpenAPI spec at a file path.
    OpenApiFile { path: String },
    /// GraphQL endpoint URL.
    GraphQL { url: String },
}

impl DiscoveredSource {
    pub fn into_backend(self) -> Box<dyn crate::backend::Backend> {
        use crate::backend::mcp::McpBackend;
        match self.kind {
            BackendKind::McpStdio { command } => Box::new(McpBackend::from_stdio(command)),
            BackendKind::McpHttp { url } => Box::new(McpBackend::from_http(url, vec![])),
            BackendKind::OpenApiFile { path } => {
                use crate::backend::openapi::OpenApiBackend;
                Box::new(OpenApiBackend::from_file(&path).expect("openapi load"))
            }
            BackendKind::GraphQL { url } => {
                use crate::backend::graphql::GraphQlBackend;
                Box::new(GraphQlBackend::new(url, vec![]))
            }
        }
    }
}

/// Port: anything that can produce a list of DiscoveredSources.
#[async_trait]
pub trait SourceScanner: Send + Sync {
    async fn scan(&self) -> Vec<DiscoveredSource>;
}
