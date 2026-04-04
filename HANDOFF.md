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
- `McpBackend`: HTTP/SSE transport (`HttpSession`) — connects to SSE endpoint, waits for `endpoint` event, POSTs JSON-RPC 2.0, routes responses via background stream task + oneshot channels
- Verified against Pieces MCP (`http://localhost:39300/model_context_protocol/2024-11-05/sse`): ~35 tools discovered, execute round-trip confirmed

---

## Known Gaps / Remaining Work

### 1. Relative `servers[0].url` in OpenAPI specs

**Problem:** Specs like the petstore use `/api/v3` as `servers[0].url`. When mcpipe fetches the spec from a URL, it uses this path literally as the base URL, which fails the HTTP client.

**Fix:** Add a `--base-url <URL>` global flag. In `OpenApiBackend::execute()`, prefer the user-supplied base URL over `servers[0].url`. Also consider auto-resolving a relative server URL against the spec's fetch URL (strip path, append relative URL).

**Files:** `src/backend/openapi.rs` (`execute` → HTTP client base), `src/main.rs` (parse + thread through to backend construction).

---

### ~~2. MCP HTTP/SSE transport~~ ✅ DONE (2026-04-03)

`HttpSession` implemented in `src/backend/mcp.rs`. Connects to SSE endpoint, waits for `endpoint` event, POSTs JSON-RPC 2.0 via `reqwest`, routes responses from background SSE stream via oneshot channels. Verified against Pieces MCP.

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

### ~~6. `--list`/`--search` output polish~~ ✅ DONE (2026-04-03)

Aligned two-column output with description; case-insensitive `--search` working.

---

## scan-all Feature — 2026-04-04 ✅ COMPLETE

The `--scan` / scan-all feature is fully implemented and all 35 tests pass.

### What was built

- `DiscoveredSource` domain type and `SourceScanner` port (`src/domain.rs`)
- `ClaudeConfigScanner` — reads Claude Desktop/Code MCP server config files
- `WorkspaceScanner` — finds OpenAPI specs (JSON/YAML) in a workspace directory
- `WellKnownScanner` — probes local HTTP endpoints for live MCP servers (Pieces MCP, etc.)
- `--scan` flag in `main.rs` wiring all scanners into a unified discovery report

### Known gap — deferred

**GraphQL endpoint heuristics** are not implemented. There is no reliable way to detect a GraphQL
endpoint without sending a speculative introspection query, which has side effects on some servers.
Deferred until a safe probe strategy is identified (e.g. OPTIONS + content-type check before
committing to introspection).

---

## Sentinel Review — 2026-04-03

### Blocking

- [`src/main.rs:60` / `src/backend/mcp.rs:19`] `McpTransport::Http` field named `auth_headers` but receives the merged `all_headers` (auth + user `--header` values). Naming mismatch will cause maintenance bugs — rename field to `headers`.
- [`src/backend/mcp.rs:138,171`] `_stream_task` `JoinHandle` is never `.await`ed or `.abort()`ed on drop. If the SSE stream task panics, in-flight `oneshot::Sender`s are abandoned and the session is silently dead for its lifetime. Add an abort handle or structured shutdown signal.

### Suggestions

- [`src/main.rs:158–184`] Manual argv stripping for global flags must be kept in sync with `build_global_parser` by hand. A mismatch silently passes unknown flags into the dynamic subcommand parser. Consider deriving the strip list from the parser or adding a round-trip test.
- [`src/main.rs:77–94`] Relative-URL resolution logic is duplicated between `main.rs` and `mcp.rs`. Extract a `resolve_relative_url(base, path) -> String` utility in `src/lib.rs`.
- [`src/backend/openapi.rs:180`] `reqwest::Client::new()` on every `execute` call allocates a new connection pool. Build the client once in `from_file`/`from_json` and store it on the struct.
- [`src/backend/mcp.rs:44`] Child stderr discarded via `Stdio::null()`. Stdio server diagnostics are invisible on crash. Consider forwarding to `eprintln!` via a background reader.
- [`src/backend/openapi.rs:174`] `ParamLocation::Header` params parsed from spec but never injected into outgoing requests — add a `// TODO` comment so it's not mistaken for intentional behavior.
