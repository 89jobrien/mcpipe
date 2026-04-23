# mcpipe

## Install

```bash
cargo install --path .
mcpipe --version
```

Binary installs to `~/.cargo/bin/mcpipe` (ensure `~/.cargo/bin` is on PATH).

## Build & Run

- `cargo build --release` — binary at `./target/release/mcpipe`
- `cargo clippy && cargo test` — required before commit
- `cargo test --features integration` — run integration tests (requires live MCP servers)

## Environment Variables

- `MCPIPE_LOG` — set to `debug` or `trace` for verbose output (default: `info`)
- `MCPIPE_TIMEOUT` — request timeout in seconds (default: 30)

## Common Commands

- `./target/release/mcpipe --mcp <SSE_URL> --list` — discover tools from an MCP server
- `./target/release/mcpipe --mcp <SSE_URL> <tool-name> --<param> <value>` — execute a tool
- Pieces MCP SSE endpoint: `http://localhost:39300/model_context_protocol/2024-11-05/sse`

## PathBinaryScanner Auto-Discovery

`PathBinaryScanner` in `src/discovery.rs` scans the system PATH for executables and auto-generates
tool definitions based on their help output. Invoked via `--scan-path <prefix>` flag.

- Discovers tools matching executable name prefix (e.g. `--scan-path rustc` finds rustc-related tools)
- Parses `--help` output to extract parameters and descriptions
- Generates `CommandDef` structs dynamically — tools don't need explicit registration

## Architecture

- `src/backend/mcp.rs` — stdio + HTTP/SSE transports; `StdioSession` and `HttpSession`
- `src/backend/openapi.rs`, `src/backend/graphql.rs` — other backend types
- `src/domain.rs` — `Backend` trait (hexagonal port), `CommandDef`, `BackendError`
- `src/main.rs` — dynamic clap CLI built from discovered `CommandDef`s at runtime

## Quirks & Notes

- `--list` output includes descriptions; to extract tool names: `rg '^[a-z][a-z0-9-]+\s' | cut -d' ' -f1`
- Pieces MCP exposes ~35 real tools (full-text search, vector search, batch snapshot, LTM)
- Maestro API spec + docs: `maestro-api/maestro-api.openapi.yaml` and `maestro-api/API.md`
- `HANDOFF.mcpipe.workspace.yaml` tracks open items; sync with
  `doob handoff sync --file HANDOFF.mcpipe.workspace.yaml`
