# mcpipe

Rust 2024 CLI and library that converts MCP, OpenAPI, GraphQL, and compatible CLI schemas into
shell-callable commands.

## Build and Verify

```bash
cargo build --release
cargo install --path .
mcpipe --version

cargo fmt --all
cargo clippy --workspace -- -D warnings
cargo nextest run --workspace
```

Live-service tests are feature-gated:

```bash
cargo nextest run --workspace --features integration
```

## Environment

| Variable           | Behavior                                         |
| ------------------ | ------------------------------------------------ |
| `MCPIPE_CACHE_DIR` | Overrides the platform discovery-cache directory |

Request and scan timeouts are currently fixed in code. There is no logging environment variable.

## CLI Surface

Use `mcpipe --help` as the source of truth for global flags. The only static subcommand is
`completions`; backend operations are discovered at runtime.

```bash
mcpipe --spec tests/fixtures/petstore.json --list
mcpipe --spec tests/fixtures/petstore.json show-pet-by-id --help
mcpipe completions
```

`--fields` is a GraphQL selection-set override. It appears on generated command help for every
backend because the command tree is backend-neutral; non-GraphQL adapters ignore it.

`--gen-openapi` requires `--cli`. It writes to `--openapi-output` when supplied, otherwise to:

```text
$HOME/.ctx/mcpipe/schemas/openapi
```

## Architecture

The `Backend` port is defined in `src/backend/mod.rs`. Adapters discover `CommandDef` values and
execute one selected command. `src/main.rs` is the composition root and owns global parsing,
backend selection, caching, dynamic parsing, execution, and output.

| Module                   | Responsibility                                                   |
| ------------------------ | ---------------------------------------------------------------- |
| `src/backend/mod.rs`     | `Backend` trait                                                  |
| `src/backend/mcp.rs`     | MCP stdio and HTTP/SSE adapter                                   |
| `src/backend/openapi.rs` | OpenAPI loading, reference resolution, and HTTP execution        |
| `src/backend/graphql.rs` | GraphQL introspection and execution                              |
| `src/backend/cli.rs`     | `schema --json` CLI discovery and subprocess execution           |
| `src/domain.rs`          | Command, parameter, location, argument, and backend-error types  |
| `src/cli.rs`             | Dynamic Clap command construction and argument conversion        |
| `src/deser.rs`           | JSON, YAML, TOML, and JSON5 parsing                              |
| `src/discovery.rs`       | Discovered-source types and `SourceScanner` port                 |
| `src/scanner/`           | Claude config, workspace, well-known endpoint, and PATH scanners |
| `src/cache.rs`           | TTL cache for discovered command definitions                     |
| `src/format.rs`          | Compact, pretty, raw, jq, and array-head output handling         |
| `src/openapi_gen.rs`     | OpenAPI 3.1 generation from command definitions                  |
| `src/secret.rs`          | `env:`, `file:`, and literal auth-header values                  |
| `src/main.rs`            | Binary composition root                                          |

The PATH scanner is `PathBinaryScanner` in `src/scanner/path_binary.rs`. It runs as part of
`mcpipe --scan` and checks a fixed registry of known MCP binaries; it does not accept a prefix or
derive schemas from arbitrary executable help output.

## Public API

`src/lib.rs` exports ten modules. Every public type and function is listed in `docs/API.md`.

## Handoff

Project handoff state is stored in:

```text
.ctx/HANDOFF.mcpipe.mcpipe.yaml
```

Read the Markdown status in `HANDOFF.md`; synchronize structured state with `hj handoff`.
