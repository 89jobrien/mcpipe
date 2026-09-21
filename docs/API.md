# mcpipe Public API

This reference covers every module exported by `src/lib.rs` and every public item declared under
those modules. Signatures are abbreviated only where Rust syntax would repeat field bodies already
listed in a table.

## `backend`

The backend port is:

```rust
#[async_trait]
pub trait Backend: Send + Sync {
    async fn discover(&self) -> Result<Vec<CommandDef>, BackendError>;
    async fn execute(
        &self,
        cmd: &CommandDef,
        args: ArgMap,
    ) -> Result<serde_json::Value, BackendError>;
}
```

`discover` returns the commands exposed by one source. `execute` invokes one discovered command
with values keyed by the parameter's original source name.

### `backend::mcp::McpBackend`

| Signature                                                                    | Purpose                             |
| ---------------------------------------------------------------------------- | ----------------------------------- |
| `McpBackend::from_stdio(command: String) -> Self`                            | Configure an MCP subprocess command |
| `McpBackend::from_http(url: String, headers: Vec<(String, String)>) -> Self` | Configure an MCP HTTP/SSE endpoint  |

```rust
use mcpipe::backend::mcp::McpBackend;

let backend = McpBackend::from_stdio("python tests/fixtures/mcp_echo.py".to_string());
```

### `backend::openapi::OpenApiBackend`

| Signature                                                         | Purpose                                              |
| ----------------------------------------------------------------- | ---------------------------------------------------- |
| `OpenApiBackend::from_file(path: &str) -> anyhow::Result<Self>`   | Load and parse a local spec                          |
| `OpenApiBackend::from_json(spec, base_url, auth_headers) -> Self` | Construct from parsed JSON                           |
| `with_base_url(self, base_url: String) -> Self`                   | Replace the extracted server URL                     |
| `with_auth_headers(self, headers: Vec<(String, String)>) -> Self` | Replace request headers                              |
| `resolve_refs(spec: &serde_json::Value) -> serde_json::Value`     | Inline local JSON-pointer refs                       |
| `to_kebab(value: &str) -> String`                                 | Convert an operation or parameter name to kebab case |

`resolve_refs` leaves circular or unresolved references in place. `from_file` accepts JSON, YAML,
TOML, and JSON5 through the `deser` module.

```rust
use mcpipe::backend::openapi::OpenApiBackend;

let backend = OpenApiBackend::from_file("tests/fixtures/petstore.json")?
    .with_base_url("http://localhost:8080".to_string());
# Ok::<(), anyhow::Error>(())
```

### `backend::graphql::GraphQlBackend`

| Signature                                                                            | Purpose                          |
| ------------------------------------------------------------------------------------ | -------------------------------- |
| `GraphQlBackend::new(endpoint: String, auth_headers: Vec<(String, String)>) -> Self` | Configure live introspection     |
| `GraphQlBackend::from_introspection(endpoint, introspection, auth_headers) -> Self`  | Use supplied introspection JSON  |
| `with_fields_override(self, fields: String) -> Self`                                 | Set the default selection fields |

```rust
use mcpipe::backend::graphql::GraphQlBackend;

let backend = GraphQlBackend::new("http://localhost:4000/graphql".to_string(), vec![])
    .with_fields_override("id name".to_string());
```

### `backend::cli::CliBackend`

| Signature                                             | Purpose                                      |
| ----------------------------------------------------- | -------------------------------------------- |
| `CliBackend::new(command: impl Into<String>) -> Self` | Configure a command exposing `schema --json` |

```rust
use mcpipe::backend::cli::CliBackend;

let backend = CliBackend::new("doob");
```

## `cache`

`Cache` stores serialized `Vec<CommandDef>` values. Its fields are private.

| Signature                                                                  | Purpose                                                     |
| -------------------------------------------------------------------------- | ----------------------------------------------------------- |
| `Cache::new(dir: PathBuf, ttl: Duration) -> Self`                          | Configure storage and expiration                            |
| `Cache::default_dir() -> PathBuf`                                          | Read `MCPIPE_CACHE_DIR` or use the platform cache directory |
| `load(&self, source: &str) -> Option<Vec<CommandDef>>`                     | Return an unexpired cached schema                           |
| `save(&self, source: &str, commands: &[CommandDef]) -> anyhow::Result<()>` | Persist a schema                                            |

```rust
use mcpipe::cache::Cache;
use std::time::Duration;

let cache = Cache::new(Cache::default_dir(), Duration::from_secs(3600));
let commands = cache.load("https://example.test/openapi.json");
```

## `cli`

| Signature                                                                 | Purpose                              |
| ------------------------------------------------------------------------- | ------------------------------------ |
| `build_command(app_name: &str, commands: &[CommandDef]) -> clap::Command` | Build discovered subcommands         |
| `extract_args(matches: &ArgMatches, command: &CommandDef) -> ArgMap`      | Convert matched flags to JSON values |

`build_command` adds a `--fields` option to generated commands. `extract_args` stores that value
under the internal `__fields` key used by the GraphQL adapter.

```rust
use mcpipe::cli::build_command;

let app = build_command("mcpipe", &[]);
assert_eq!(app.get_name(), "mcpipe");
```

## `deser`

`FormatHint` selects `Json`, `Yaml`, `Toml`, `Json5`, or `Unknown` parsing.

| Signature                                                                        | Purpose                                 |
| -------------------------------------------------------------------------------- | --------------------------------------- |
| `FormatHint::from_extension(ext: &str) -> Self`                                  | Derive a hint from a file extension     |
| `FormatHint::from_content_type(content_type: &str) -> Self`                      | Derive a hint from an HTTP content type |
| `parse_any(bytes: &[u8], hint: FormatHint) -> anyhow::Result<serde_json::Value>` | Parse supported text into JSON          |

When the hint is `Unknown`, `parse_any` tries JSON, YAML, TOML, then JSON5.

```rust
use mcpipe::deser::{FormatHint, parse_any};

let value = parse_any(br#"{"name":"mcpipe"}"#, FormatHint::Json)?;
assert_eq!(value["name"], "mcpipe");
# Ok::<(), anyhow::Error>(())
```

## `discovery`

```rust
pub struct DiscoveredSource {
    pub name: String,
    pub kind: BackendKind,
    pub origin: String,
}

pub enum BackendKind {
    McpStdio { command: String },
    McpHttp { url: String },
    OpenApiFile { path: String },
    GraphQL { url: String },
    Cli { command: String },
}
```

| Signature                                                                  | Purpose                                |
| -------------------------------------------------------------------------- | -------------------------------------- |
| `DiscoveredSource::into_backend(self) -> anyhow::Result<Box<dyn Backend>>` | Construct the selected adapter         |
| `SourceScanner::scan(&self) -> Vec<DiscoveredSource>`                      | Discover configured source definitions |

`SourceScanner` is an async `Send + Sync` trait.

```rust
use mcpipe::discovery::{BackendKind, DiscoveredSource};

let source = DiscoveredSource {
    name: "local".to_string(),
    kind: BackendKind::McpStdio {
        command: "server mcp".to_string(),
    },
    origin: "manual".to_string(),
};
let backend = source.into_backend()?;
# Ok::<(), anyhow::Error>(())
```

## `domain`

```rust
pub struct CommandDef {
    pub name: String,
    pub description: String,
    pub params: Vec<ParamDef>,
    pub source_name: String,
}

pub struct ParamDef {
    pub name: String,
    pub original_name: String,
    pub required: bool,
    pub description: String,
    pub location: ParamLocation,
    pub schema: serde_json::Value,
}
```

`ParamLocation` has `Body`, `Query`, `Path`, `Header`, and `ToolInput` variants. `ArgMap` is a
`HashMap<String, serde_json::Value>`.

`BackendError` is an error enum with `Discovery`, `Execution`, `NotFound`, `Transport`, and
`Schema` string variants.

```rust
use mcpipe::domain::{CommandDef, ParamDef, ParamLocation};

let command = CommandDef {
    name: "list-pets".to_string(),
    description: "List all pets".to_string(),
    params: vec![ParamDef {
        name: "limit".to_string(),
        original_name: "limit".to_string(),
        required: false,
        description: "How many items to return".to_string(),
        location: ParamLocation::Query,
        schema: serde_json::json!({"type": "integer"}),
    }],
    source_name: "listPets".to_string(),
};
```

## `format`

```rust
pub struct FormatOptions {
    pub pretty: bool,
    pub raw: bool,
    pub jq: Option<String>,
    pub head: Option<usize>,
}
```

| Signature                                                                                    | Purpose               |
| -------------------------------------------------------------------------------------------- | --------------------- |
| `format_value(value: &serde_json::Value, options: &FormatOptions) -> anyhow::Result<String>` | Format command output |

`raw` takes precedence over JSON serialization and jq. `head` only truncates arrays. `jq` requires
the external `jq` executable.

```rust
use mcpipe::format::{FormatOptions, format_value};

let output = format_value(
    &serde_json::json!([1, 2, 3]),
    &FormatOptions {
        pretty: false,
        raw: false,
        jq: None,
        head: Some(2),
    },
)?;
assert_eq!(output, "[1,2]");
# Ok::<(), anyhow::Error>(())
```

## `openapi_gen`

| Signature                                                                                | Purpose                       |
| ---------------------------------------------------------------------------------------- | ----------------------------- |
| `generate(tool_name: &str, version: &str, commands: &[CommandDef]) -> serde_json::Value` | Build an OpenAPI 3.1 document |
| `to_yaml(document: &serde_json::Value) -> Result<String, serde_yaml::Error>`             | Serialize a document as YAML  |

```rust
use mcpipe::openapi_gen::{generate, to_yaml};

let document = generate("example", "1.0.0", &[]);
let yaml = to_yaml(&document)?;
assert!(yaml.contains("openapi: 3.1.0"));
# Ok::<(), serde_yaml::Error>(())
```

## `scanner`

All scanner structs implement `SourceScanner`.

| Public item                                                  | Purpose                                                           |
| ------------------------------------------------------------ | ----------------------------------------------------------------- |
| `ClaudeConfigScanner::from_paths(settings_paths, mcp_paths)` | Scan explicit Claude and MCP config paths                         |
| `ClaudeConfigScanner::default_env()`                         | Scan `$HOME/.claude/settings.json` and projects under `$HOME/dev` |
| `WorkspaceScanner::from_roots(roots)`                        | Scan explicit roots for named OpenAPI files                       |
| `WorkspaceScanner::default_env()`                            | Scan `$HOME/dev` for named OpenAPI files                          |
| `WellKnownScanner::new()`                                    | Probe registered local HTTP services                              |
| `WellKnownScanner::default()`                                | Equivalent to `new`                                               |
| `PathBinaryScanner::new()`                                   | Search the process `PATH` for registered MCP binaries             |
| `PathBinaryScanner::with_path(path)`                         | Search an explicit platform-separated path string                 |
| `PathBinaryScanner::well_known_names()`                      | Return the registered binary-name allowlist                       |
| `PathBinaryScanner::default()`                               | Equivalent to `new`                                               |

```rust
use mcpipe::scanner::path_binary::PathBinaryScanner;

let scanner = PathBinaryScanner::with_path("/usr/local/bin:/usr/bin");
assert!(PathBinaryScanner::well_known_names().contains(&"obfsck-mcp"));
```

## `secret`

| Signature                                               | Purpose                                    |
| ------------------------------------------------------- | ------------------------------------------ |
| `resolve_secret(value: &str) -> anyhow::Result<String>` | Resolve `env:`, `file:`, or literal values |

```rust
use mcpipe::secret::resolve_secret;

assert_eq!(resolve_secret("literal-token")?, "literal-token");
# Ok::<(), anyhow::Error>(())
```
