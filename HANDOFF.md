# mcpipe — Handoff

## What's Done

- `Backend` trait with `discover()` + `execute()` — hexagonal architecture
- `McpBackend`: stdio transport (per-call subprocess spawn, JSON-RPC 2.0)
- `OpenApiBackend`: JSON/YAML/TOML/JSON5 spec loading, `$ref` resolution, operation→CommandDef mapping
- `GraphQlBackend`: introspection query, query/mutation→CommandDef mapping, auto-generated selection sets
- Dynamic clap CLI generation from discovered `CommandDef` list at runtime
- TTL-based disk cache (SHA-256 keyed, `~/.cache/mcpipe/`, `MCPIPE_CACHE_DIR` override)
- Output formatting: `--pretty`, `--raw`, `--jq`, `--head`
- Secret resolution: `env:VAR`, `file:/path`, literal passthrough
- Multi-format deserialization (`src/deser.rs`): Content-Type + URL extension hint, fallback chain
- 28 tests passing; GitHub repo at `https://github.com/89jobrien/mcpipe`

---

## Known Gaps / Remaining Work

### 1. Relative `servers[0].url` in OpenAPI specs

**Problem:** Specs like the petstore use `/api/v3` as `servers[0].url`. When mcpipe fetches the spec from a URL, it uses this path literally as the base URL, which fails the HTTP client.

**Fix:** Add a `--base-url <URL>` global flag. In `OpenApiBackend::execute()`, prefer the user-supplied base URL over `servers[0].url`. Also consider auto-resolving a relative server URL against the spec's fetch URL (strip path, append relative URL).

**Files:** `src/backend/openapi.rs` (`execute` → HTTP client base), `src/main.rs` (parse + thread through to backend construction).

---

### 2. MCP HTTP/SSE transport

**Problem:** `McpBackend::from_http()` returns `Err(BackendError::Transport("MCP HTTP transport not yet implemented"))`.

**Fix:** Implement two sub-modes:
- **HTTP**: POST JSON-RPC to `{url}/rpc` (or configurable path), use `reqwest`.
- **SSE**: Connect to `{url}/sse`, parse `eventsource-client` stream for JSON-RPC responses. `eventsource-client` is already in `Cargo.toml`.

MCP HTTP is JSON-RPC 2.0 — same message structure as stdio, different transport layer. `send_request` / `send_notification` / `send_initialize` logic can be shared with stdio.

**Files:** `src/backend/mcp.rs` — add `HttpSession` struct alongside `StdioSession`.

---

### 3. `--header` flag (arbitrary request headers)

**Problem:** No way to pass arbitrary HTTP headers. GitHub API requires `User-Agent: <app>` — currently unobtainable.

**Fix:** Add `--header KEY:VALUE` (repeatable) to the global parser in `main.rs`. Thread headers into `OpenApiBackend` and `GraphQlBackend` constructors alongside the existing auth headers. The auth header `--auth-header` already does this for one header — generalize.

**Files:** `src/main.rs`, `src/backend/openapi.rs`, `src/backend/graphql.rs`.

---

### 4. GraphQL `--fields` override

**Problem:** The spec says `--fields` overrides the auto-generated selection set, but it's not wired. The `GraphQlBackend` has `with_fields_override()` but the CLI doesn't expose it per-subcommand.

**Fix:** Add `--fields <FIELDS>` as a per-subcommand arg (or a global flag that applies to the executed command). In `GraphQlBackend::execute()`, check `args` for a `"__fields"` key and use it as the selection set string if present.

**Files:** `src/cli.rs` (inject `--fields` arg for GraphQL subcommands), `src/backend/graphql.rs` (`execute` selection set logic).

---

### 5. Integration test feature gate

**Spec says:** Integration tests behind `--features integration`, not run in CI by default. Point at a live MCP server, run discovery + execute cycle.

**Fix:** Add `[features] integration = []` to `Cargo.toml`. Gate test files with `#![cfg(feature = "integration")]`. Write at least one test that hits a real MCP server (e.g. the echo fixture server via stdio, or a real HTTP endpoint). Run with `cargo test --features integration`.

**Files:** `Cargo.toml`, `tests/mcp_adapter.rs` (or new `tests/integration/`).

---

### 6. `--list`/`--search` output polish

**Current:** Prints bare command names one per line.

**Fix:** Print `name  —  description` aligned in two columns (use `max name length` for padding). `--search PATTERN` already filters by substring — add case-insensitive match. Optional: show param count or required params summary.

**Files:** `src/main.rs` (list/search render block).
