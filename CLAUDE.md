# mcpipe

## Build & Run
- `cargo build --release` — binary at `./target/release/mcpipe`
- `cargo clippy && cargo test` — required before commit

## Common Commands
- `./target/release/mcpipe --mcp <SSE_URL> --list` — discover tools from an MCP server
- `./target/release/mcpipe --mcp <SSE_URL> <tool-name> --<param> <value>` — execute a tool
- Pieces MCP SSE endpoint: `http://localhost:39300/model_context_protocol/2024-11-05/sse`

## Architecture
- `src/backend/mcp.rs` — stdio + HTTP/SSE transports; `StdioSession` and `HttpSession`
- `src/backend/openapi.rs`, `src/backend/graphql.rs` — other backend types
- `src/domain.rs` — `Backend` trait (hexagonal port), `CommandDef`, `BackendError`
- `src/main.rs` — dynamic clap CLI built from discovered `CommandDef`s at runtime

## Quirks
- `--list` output includes descriptions; to extract tool names: `grep -E '^[a-z][a-z0-9-]+\s' | awk '{print $1}'`
- Pieces MCP exposes ~35 real tools (full-text search, vector search, batch snapshot, LTM)
- `HANDOFF.md` tracks known gaps and remaining work
