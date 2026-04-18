# mcpipe

Turn any MCP server, OpenAPI spec, or GraphQL endpoint into a shell CLI.

## Install

```bash
cargo install --path .
```

## Usage

```
mcpipe --mcp-stdio <cmd> [SUBCOMMAND] [ARGS]
mcpipe --mcp <url> [SUBCOMMAND] [ARGS]
mcpipe --spec <file-or-url> [SUBCOMMAND] [ARGS]
mcpipe --graphql <url> [SUBCOMMAND] [ARGS]
mcpipe --cli <cmd> [SUBCOMMAND] [ARGS]
```

### Backends

| Flag | Source |
|------|--------|
| `--mcp-stdio <cmd>` | MCP server over stdio |
| `--mcp <url>` | MCP server over HTTP/SSE |
| `--spec <path\|url>` | OpenAPI 3.x spec (JSON or YAML) |
| `--graphql <url>` | GraphQL endpoint (introspection) |
| `--cli <cmd>` | Existing shell CLI (help-text parsing) |

### Global flags

```
--list                List available commands and exit
--scan                Scan Claude config and workspace for MCP sources
--pretty              Pretty-print JSON output
--raw                 Output raw string values
--jq <expr>           Apply a jq-style filter to the response
--head <n>            Limit array output to first n items
--search <term>       Filter listed commands by name
--refresh             Bypass cache and re-fetch schema
--cache-ttl <secs>    Schema cache TTL in seconds (default: 3600)
--auth-header <k:v>   Auth header; value may be env:VAR or keychain:ITEM
--header <k:v>        Extra request header
--base-url <url>      Override base URL for OpenAPI backends
--gen-openapi         Generate an OpenAPI spec from a CLI backend
--openapi-output <p>  Output path for generated spec
```

### Examples

```bash
# List all operations in a local OpenAPI spec
mcpipe --spec ./petstore.json --list

# Call an MCP server tool
mcpipe --mcp-stdio "uvx my-mcp-server" call-tool --arg value

# Query a GraphQL endpoint
mcpipe --graphql https://api.example.com/graphql --list

# Wrap an existing CLI
mcpipe --cli gh --list
mcpipe --cli gh issue list

# Generate an OpenAPI spec from a CLI
mcpipe --cli mycli --gen-openapi
```

## License

MIT OR Apache-2.0
