# mcpipe — Agent Operating Guide

## Build, Test, and Lint Commands

### Quick Reference

```bash
# Build the binary
cargo build --release

# Run all tests
just ci                    # Format check + lint + tests
just pre-commit           # Format check + lint (staged files only)
cargo clippy && cargo test

# Lint code
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --all
```

### Running Tests

```bash
# Run all tests
cargo test --all
cargo nextest run --workspace

# Run integration tests (requires live MCP servers)
cargo test --features integration

# Run specific test
cargo test test_name
```

### Development Workflow

1. **Setup**: Install dependencies (`cargo build`)
2. **Development**: Edit code in `src/`
3. **Testing**: Run `just ci` locally before pushing
4. **Linting**: `cargo clippy` catches common issues
5. **Formatting**: `cargo fmt` enforces consistent style

### Installation

```bash
cargo install --path .
mcpipe --version
```

## Code Style Guidelines

### Rust Version & Edition

- **Edition**: 2024
- **Formatting width**: 100 characters (rustfmt default)
- **Components**: rustfmt, clippy

### Linting (clippy)

- Run `cargo clippy --workspace -- -D warnings` before commits
- Fix all warnings — do not suppress them

### Naming Conventions

- **Structs/Enums**: PascalCase (`CommandDef`, `BackendError`)
- **Functions/Methods/Variables**: snake_case (`run_command`, `parse_args`)
- **Constants**: SCREAMING_SNAKE_CASE
- **Modules**: snake_case (`discovery.rs`, `backend/mcp.rs`)
- **Files**: snake_case.rs

### Module Organization

- **`src/domain.rs`** — Core types (`Backend` trait, `CommandDef`, `BackendError`)
- **`src/backend/`** — Backend implementations (MCP, OpenAPI, GraphQL)
- **`src/scanner/`** — PathBinaryScanner auto-discovery logic
- **`src/cli.rs`** — Clap CLI structure
- **`src/main.rs`** — Entry point (builds dynamic CLI at runtime)
- **`src/discovery.rs`** — Tool discovery and scanning
- **`src/cache.rs`** — Result caching for tool definitions
- **`src/format.rs`** — Output formatting (JSON, plain text, etc.)

### Error Handling

- Use `anyhow::Result<T>` for application errors
- Avoid `unwrap()` and `expect()` in production code
- Propagate errors with `?` operator
- `BackendError` trait for backend-specific error types

### Testing Patterns

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // Test implementation
    }
}
```

## Project Architecture

### Hexagonal Design (Ports & Adapters)

- **Port**: `Backend` trait in `domain.rs` (abstract contract)
- **Adapters**: `mcp.rs`, `openapi.rs`, `graphql.rs` (concrete implementations)
- **CLI**: `cli.rs` with `clap` derive macros; dynamically generates subcommands at runtime

### Key Modules

| Module           | Purpose                                                  |
| ---------------- | -------------------------------------------------------- |
| `domain.rs`      | `Backend` trait, `CommandDef`, core error types          |
| `backend/mcp.rs` | MCP stdio + HTTP/SSE transports (`StdioSession`, etc.)   |
| `main.rs`        | CLI entry point; builds dynamic clap commands at runtime |
| `discovery.rs`   | Tool discovery from backends and PATH scanning           |
| `scanner/`       | PathBinaryScanner auto-discovery from system executables |

### Feature Flags

- **`integration`** — Enables integration tests requiring live MCP servers

### Environment Variables

- `MCPIPE_LOG` — Logging level (`debug`, `trace`, default: `info`)
- `MCPIPE_TIMEOUT` — Request timeout in seconds (default: 30)

## Common Commands

### Running mcpipe Locally

```bash
# Discover tools from an MCP server
./target/release/mcpipe --mcp http://localhost:39300/.../sse --list

# Execute a tool
./target/release/mcpipe --mcp <URL> <tool-name> --<param> <value>

# Scan system PATH for executables
./target/release/mcpipe --scan-path rustc
```

### Pieces MCP Example

```bash
# Pieces MCP SSE endpoint
PIECES_URL="http://localhost:39300/model_context_protocol/2024-11-05/sse"

# List available tools
mcpipe --mcp $PIECES_URL --list

# Full-text search
mcpipe --mcp $PIECES_URL search_documents --query "authentication"

# Vector search
mcpipe --mcp $PIECES_URL search_by_vector --query "auth patterns"
```

## Commit Guidelines

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

**Types**:

- `feat:` New features
- `fix:` Bug fixes
- `docs:` Documentation
- `refactor:` Code restructuring
- `test:` Testing changes
- `chore:` Maintenance

**Examples**:

- `feat(backend): add GraphQL subscription support`
- `fix(scanner): handle missing executable permissions`
- `docs: update environment variable reference`

## Key Dependencies

| Dependency            | Purpose                                      |
| --------------------- | -------------------------------------------- |
| `clap`                | CLI argument parsing (derive macros)         |
| `tokio`               | Async runtime                                |
| `reqwest`             | HTTP client with streaming support           |
| `serde`, `serde_json` | Serialization/deserialization                |
| `eventsource-client`  | SSE (Server-Sent Events) client for MCP HTTP |
| `anyhow`              | Error handling and propagation               |
| `thiserror`           | Custom error type derivation                 |

## Quick Troubleshooting

| Issue                          | Fix                                                 |
| ------------------------------ | --------------------------------------------------- |
| Clippy warnings on commit      | Run `cargo clippy -- -D warnings` and fix issues    |
| Test failures (integration)    | Ensure MCP servers are running; test with default   |
| Build fails with locked deps   | Run `cargo build --locked` or update `Cargo.lock`   |
| Binary not on PATH after build | Run `cargo install --path .` to install to ~/.cargo |

## Testing Strategy

- **Unit tests** in source files (`mod tests {}`)
- **Integration tests** in `tests/` directory (require feature flag)
- **Fixtures** in `tests/fixtures/` (JSON specs, mock MCP servers)
- Run `just ci` to mirror GitHub Actions locally
