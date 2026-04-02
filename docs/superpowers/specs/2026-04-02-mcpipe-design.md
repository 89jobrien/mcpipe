# mcpipe — Design Spec

**Date:** 2026-04-02
**Status:** Approved

## Overview

`mcpipe` is a standalone Rust CLI binary that turns any MCP server, OpenAPI spec, or GraphQL endpoint into shell-callable subcommands. It is a Rust rewrite of [mcp2cli](https://pypi.org/project/mcp2cli/), motivated by the desire to own and maintain the tool, ship a single binary with no Python/uvx dependency, and build a clean extensible architecture from the start.

## Goals

- Single binary, no runtime dependencies
- Support all three source modes: MCP (stdio + HTTP/SSE), OpenAPI, GraphQL
- Dynamic CLI generation from discovered operations at runtime
- TTL-based disk cache for discovered schemas
- Output formatting: `--pretty`, `--raw`, `--jq`, `--head`
- Secret resolution: `env:VAR` and `file:/path` prefixes on auth headers
- v1 scope: no OAuth, no session daemon (deferred to v2)

## Architecture

Hexagonal architecture with a `Backend` trait as the central port. The CLI layer is generic over backends. Dependencies point inward: adapters depend on domain types, domain has zero external dependencies.

```
main.rs (composition root)
  │
  ├─ parse global flags
  ├─ build Box<dyn Backend>
  ├─ backend.discover() → Vec<CommandDef>
  ├─ cli.rs: build clap::Command tree
  ├─ clap parses remaining argv
  └─ backend.execute(cmd, args) → Value → format.rs → stdout
```

## Directory Structure

```
mcpipe/
├── src/
│   ├── main.rs           # composition root: flag parse, backend dispatch
│   ├── domain.rs         # CommandDef, ParamDef, ArgMap, BackendError
│   ├── backend/
│   │   ├── mod.rs        # Backend trait
│   │   ├── mcp.rs        # MCP stdio + HTTP/SSE
│   │   ├── openapi.rs    # OpenAPI spec loader + command gen
│   │   └── graphql.rs    # GraphQL introspection + command gen
│   ├── cli.rs            # dynamic clap command builder from CommandDef list
│   ├── cache.rs          # TTL disk cache (JSON, keyed by SHA-256)
│   ├── format.rs         # output formatting (pretty/raw/jq/head)
│   └── secret.rs         # env:/file: secret resolution
├── tests/
│   ├── mcp_adapter.rs    # adapter tests: spawn echo MCP server, roundtrip
│   ├── openapi_adapter.rs # fixture spec files → CommandDef assertions
│   └── graphql_adapter.rs # fixture introspection JSON → CommandDef assertions
└── Cargo.toml
```

## Domain Types

All types in `domain.rs`. No external crate dependencies in this module.

```rust
pub struct CommandDef {
    pub name: String,
    pub description: String,
    pub params: Vec<ParamDef>,
    // MCP tool name, OpenAPI operation id, or GraphQL field name
    pub source_name: String,
}

pub struct ParamDef {
    pub name: String,         // kebab-case CLI flag
    pub original_name: String,
    pub required: bool,
    pub description: String,
    pub location: ParamLocation, // Body | Query | Path | Header | ToolInput
    pub schema: serde_json::Value,
}

pub enum ParamLocation {
    Body,
    Query,
    Path,
    Header,
    ToolInput,
}

pub type ArgMap = std::collections::HashMap<String, serde_json::Value>;

pub enum BackendError {
    Discovery(String),   // failed to fetch tool list
    Execution(String),   // tool call failed
    NotFound(String),    // unknown command name
    Transport(String),   // network/process I/O failure
    Schema(String),      // malformed spec/schema
}
```

## Backend Trait (Port)

Defined in `backend/mod.rs`:

```rust
#[async_trait]
pub trait Backend {
    async fn discover(&self) -> Result<Vec<CommandDef>, BackendError>;
    async fn execute(&self, cmd: &CommandDef, args: ArgMap) -> Result<serde_json::Value, BackendError>;
}
```

`main.rs` holds `Box<dyn Backend>` — no generic parameter explosion.

## Adapters

### McpBackend (`backend/mcp.rs`)

Two transport modes selected at construction time:

- **Stdio**: spawns command via `tokio::process::Command`, writes JSON-RPC over stdin, reads from stdout
- **HTTP/SSE**: `reqwest` for HTTP, `eventsource-client` for SSE streaming

MCP protocol is JSON-RPC 2.0. `discover()` sends `tools/list`, `execute()` sends `tools/call`.

Stdio MCP is never cached — the subprocess is ephemeral and fast to start.

### OpenApiBackend (`backend/openapi.rs`)

- Fetches spec from URL or file path
- Resolves `$ref` pointers inline
- Maps each operation (`GET /foo`, `POST /bar`) to a `CommandDef`
- Path/query/header/body params mapped to `ParamDef` with correct `ParamLocation`
- Hand-rolled JSON traversal — no heavy openapi crate

### GraphQlBackend (`backend/graphql.rs`)

- Sends standard introspection query (`__schema`) over HTTP POST
- Maps query and mutation fields to `CommandDef`
- Builds a minimal selection set from the return type for execution
- `--fields` flag overrides auto-generated selection set

## CLI Generation (`cli.rs`)

Walks `Vec<CommandDef>` at runtime, builds a `clap::Command` tree:

- Each `CommandDef` → one subcommand
- Each required `ParamDef` → positional or `--flag` (required)
- Each optional `ParamDef` → `--flag` (optional)
- Boolean params → `--flag` (store_true)
- `--list` / `--search PATTERN` list/filter available subcommands without executing

## Caching (`cache.rs`)

- Location: `~/.cache/mcpipe/<key>.json`, overridable via `MCPIPE_CACHE_DIR`
- Key: first 16 hex chars of SHA-256 of the source URL or command string
- Stores serialized `Vec<CommandDef>`
- TTL: 3600s default, overridable via `--cache-ttl`
- `--refresh` bypasses cache
- Only caches HTTP MCP, OpenAPI, and GraphQL sources — stdio MCP always re-discovers

## Output Formatting (`format.rs`)

Applied after `execute()` returns a `serde_json::Value`:

- Default: compact JSON when piped, pretty-printed when stdout is a TTY
- `--pretty`: force pretty-print
- `--raw`: print string values without JSON encoding
- `--head N`: truncate arrays to first N elements
- `--jq EXPR`: pipe through `jq` subprocess (requires `jq` on PATH)

## Secret Resolution (`secret.rs`)

Auth header values support three forms:

- `env:VAR_NAME` — read from environment variable
- `file:/path/to/file` — read from file, strip trailing newline
- Literal string — used as-is

## Error Handling

All errors map to `BackendError` at the adapter boundary. `main.rs` matches on variant, prints to stderr, exits 1. No panics in production paths. `anyhow` used internally within adapters for ergonomic `?` chaining; converted to `BackendError` before crossing the trait boundary.

## Dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "stream"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
async-trait = "0.1"
anyhow = "1"
sha2 = "0.10"
eventsource-client = "0.12"
```

## Testing Strategy

**Unit tests** (inline `#[cfg(test)]` modules):
- `CommandDef` building from schema fixtures
- CLI flag generation from `CommandDef` list
- `format.rs` output transformations
- `cache.rs` TTL expiry against tmp dir

**Adapter tests** (`tests/`):
- `McpBackend`: spawn minimal MCP echo server as child process, assert `discover()` + `execute()` roundtrip
- `OpenApiBackend`: load petstore-style fixture JSON, assert `CommandDef` output
- `GraphQlBackend`: fixture introspection response, assert command generation

**Integration tests** (behind `--features integration`, not run in CI by default):
- Point at a live MCP server, run discovery + execute cycle

Test doubles are plain structs implementing `Backend` — no mocking frameworks.

## Out of Scope (v1)

- OAuth (PKCE, client credentials)
- Session daemon (persistent background process)
- TOON output format
- `bake` config (include/exclude lists)

These may be added in v2 without architectural changes — the `Backend` trait and CLI layer are already structured to support them.
