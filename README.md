# mcpipe

`mcpipe` turns discovered MCP tools, OpenAPI operations, GraphQL fields, and compatible CLI
commands into shell subcommands. It is a Rust 2024 binary with a shared library for backend,
discovery, formatting, and schema-generation logic.

## Install

```bash
cargo install --path .
mcpipe --version
```

## Quick Start

List operations from the included OpenAPI fixture:

```bash
mcpipe --spec tests/fixtures/petstore.json --list
```

Inspect a generated operation:

```bash
mcpipe --spec tests/fixtures/petstore.json show-pet-by-id --help
```

Use the included MCP stdio fixture:

```bash
mcpipe --mcp-stdio "python tests/fixtures/mcp_echo.py" --list
```

## Backends

| Source       | Selector               | Discovery and execution                                             |
| ------------ | ---------------------- | ------------------------------------------------------------------- |
| MCP stdio    | `--mcp-stdio <CMD>`    | Spawns the command for MCP JSON-RPC discovery or calls              |
| MCP HTTP/SSE | `--mcp <URL>`          | Reads SSE responses and posts JSON-RPC requests                     |
| OpenAPI      | `--spec <URL_OR_FILE>` | Loads JSON, YAML, TOML, or JSON5 and executes HTTP operations       |
| GraphQL      | `--graphql <URL>`      | Introspects query and mutation fields, then posts GraphQL requests  |
| CLI schema   | `--cli <COMMAND>`      | Runs `<COMMAND> schema --json`, then invokes discovered subcommands |

Exactly one source selector is required unless `--scan` is used.

## CLI Reference

| Option                       | Purpose                                                      |
| ---------------------------- | ------------------------------------------------------------ |
| `--mcp-stdio <CMD>`          | Use an MCP stdio server command                              |
| `--mcp <URL>`                | Use an MCP HTTP/SSE endpoint                                 |
| `--spec <URL_OR_FILE>`       | Use an OpenAPI spec URL or file                              |
| `--graphql <URL>`            | Use a GraphQL endpoint                                       |
| `--cli <COMMAND>`            | Use a CLI that implements `schema --json`                    |
| `--auth-header <NAME:VALUE>` | Add a repeatable header with secret resolution               |
| `--header <KEY:VALUE>`       | Add a repeatable literal HTTP header                         |
| `--base-url <URL>`           | Override an OpenAPI spec's server URL                        |
| `--list`                     | List discovered commands                                     |
| `--search <PATTERN>`         | Filter commands by name or description                       |
| `--refresh`                  | Bypass the discovery cache                                   |
| `--cache-ttl <SECS>`         | Set discovery-cache lifetime; default is 3600 seconds        |
| `--pretty`                   | Pretty-print JSON output                                     |
| `--raw`                      | Print string values without JSON quoting                     |
| `--jq <EXPR>`                | Process JSON through the installed `jq` command              |
| `--head <N>`                 | Keep the first N elements of array output                    |
| `--fields <FIELDS>`          | Override GraphQL selection fields                            |
| `--scan`                     | Discover configured, workspace, well-known, and PATH sources |
| `--gen-openapi`              | Generate an OpenAPI 3.1 document from a CLI backend          |
| `--openapi-output <FILE>`    | Choose the generated OpenAPI output file                     |
| `completions`                | Generate Nushell completions                                 |
| `-V`, `--version`            | Print the package version                                    |
| `-h`, `--help`               | Print help                                                   |

Discovered commands receive flags derived from their schemas. Boolean parameters are switches;
integer, number, array, and object values are converted from their command-line strings. Generated
commands also display `--fields`; only the GraphQL backend consumes that override.

## OpenAPI Generation

`--gen-openapi` requires a CLI backend. With no explicit output path, mcpipe writes a timestamped
file below the user's schema directory and prints the path to stderr.

```bash
mcpipe --cli doob --gen-openapi
mcpipe --cli doob --gen-openapi --openapi-output doob.openapi.yaml
```

The default schema directory is:

```text
$HOME/.ctx/mcpipe/schemas/openapi
```

## Authentication

`--auth-header` accepts these value forms:

| Form                  | Behavior                                           |
| --------------------- | -------------------------------------------------- |
| `env:VARIABLE`        | Read the value from an environment variable        |
| `file:/absolute/path` | Read the file and remove trailing CR/LF characters |
| Any other value       | Pass the value through literally                   |

```bash
mcpipe --spec api.yaml --auth-header "Authorization:env:API_TOKEN" --list
mcpipe --spec api.yaml --auth-header "Authorization:file:/run/secrets/token" --list
```

## Scanning

```bash
mcpipe --scan
```

Scanning combines four sources:

- Claude settings and project MCP configuration files under `$HOME/dev`
- OpenAPI files named `openapi.*` or `swagger.*` under `$HOME/dev`
- A health probe for a local Pieces MCP server
- Registered MCP binaries found on `PATH`; currently `obfsck-mcp`

For each source that supports discovery, scanning prints its command catalog and writes a generated
OpenAPI document below `$HOME/.ctx/mcpipe/schemas/openapi`.

## Cache and Environment

HTTP MCP, OpenAPI, and GraphQL discovery results are cached in the platform cache directory. Stdio
MCP and CLI schema discovery are not cached.

| Variable           | Purpose                                |
| ------------------ | -------------------------------------- |
| `MCPIPE_CACHE_DIR` | Override the discovery-cache directory |

## Development

```bash
cargo build
cargo fmt --all
cargo clippy --workspace -- -D warnings
cargo nextest run --workspace
```

Tests requiring live external services are gated by the `integration` feature:

```bash
cargo nextest run --workspace --features integration
```

## Documentation

- Public API: `docs/API.md`
- Design record: `docs/superpowers/specs/2026-04-02-mcpipe-design.md`
- Agent guide: `AGENTS.md`

## License

MIT OR Apache-2.0
