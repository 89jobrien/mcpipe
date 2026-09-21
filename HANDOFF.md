# mcpipe Handoff

## Implemented

- `Backend` port with MCP stdio, MCP HTTP/SSE, OpenAPI, GraphQL, and CLI adapters
- Dynamic Clap subcommands generated from discovered `CommandDef` values
- JSON, YAML, TOML, and JSON5 OpenAPI loading with local `$ref` resolution
- OpenAPI URL fetching, relative server URL resolution, base URL override, and request headers
- GraphQL introspection plus global and generated-command `--fields` overrides
- CLI schema discovery through `<command> schema --json`
- OpenAPI 3.1 generation from discovered CLI commands
- Platform cache directory with TTL, refresh, and `MCPIPE_CACHE_DIR` override
- Compact, pretty, raw, jq, and array-head output modes
- `env:`, `file:`, and literal auth-header secret values
- Claude config, workspace OpenAPI, well-known endpoint, and registered PATH scanning
- Nushell completion generation through `mcpipe completions`
- Package version output through `mcpipe --version`
- 49 default-feature tests registered across unit and integration-test binaries

## Current Limitations

- `--scan` does not attempt speculative GraphQL introspection.
- `CliBackend` supports command names with at most one nested separator, such as `todo-list`.
- Scan and MCP request timeouts are fixed in code rather than configurable through environment
  variables.
- Generated commands show `--fields` for every backend, though only GraphQL uses it.

## Key Paths

```text
src/backend/mod.rs
src/backend/mcp.rs
src/backend/openapi.rs
src/backend/graphql.rs
src/backend/cli.rs
src/discovery.rs
src/scanner/
src/openapi_gen.rs
```

Structured handoff state is stored in:

```text
.ctx/HANDOFF.mcpipe.mcpipe.yaml
```
