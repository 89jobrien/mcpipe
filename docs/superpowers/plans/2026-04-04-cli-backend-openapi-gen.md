# CLI Backend + OpenAPI Generator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `CliBackend` to mcpipe that discovers commands via `<tool> schema --json`,
executes them as subprocesses, and can generate an OpenAPI 3.1 spec from any discovered surface.
Add a `doob schema` command to the doob repo as the first target.

**Architecture:** Three independent pieces — (1) `doob schema` in the doob repo emits a
machine-readable JSON manifest of all CLI commands and params; (2) `CliBackend` in mcpipe
implements the `Backend` trait by running `<tool> schema --json` then dispatching subcommands
as subprocesses; (3) `openapi_gen` module in mcpipe walks `Vec<CommandDef>` and emits an
OpenAPI 3.1 document, exposed via `mcpipe --cli doob --gen-openapi`.

**Tech Stack:** Rust, tokio, serde_json, async-trait, clap (doob); existing mcpipe Backend
trait; serde_yaml for OpenAPI output.

---

## File Map

### doob repo (`/Users/joe/dev/doob`)

| File | Action | Responsibility |
|------|--------|---------------|
| `src/commands/schema.rs` | Create | Builds and serializes the full CLI manifest |
| `src/commands/mod.rs` | Modify | Expose `schema` module |
| `src/cli.rs` | Modify | Add `Schema` variant to `Commands` enum |
| `src/main.rs` | Modify | Handle `Commands::Schema` arm |

### mcpipe repo (`/Users/joe/dev/mcpipe`)

| File | Action | Responsibility |
|------|--------|---------------|
| `src/backend/cli.rs` | Create | `CliBackend` — runs schema + dispatches subcommands |
| `src/backend/mod.rs` | Modify | Expose `cli` module |
| `src/discovery.rs` | Modify | Add `BackendKind::Cli { command: String }` variant |
| `src/openapi_gen.rs` | Create | Walk `Vec<CommandDef>` → OpenAPI 3.1 document |
| `src/lib.rs` | Modify | Expose `openapi_gen` module |
| `src/main.rs` | Modify | Add `--cli` flag + `--gen-openapi` flag, wire `CliBackend` |
| `tests/cli_backend.rs` | Create | Integration tests (behind `integration` feature) |

---

## Task 1: `doob schema` — manifest types and serialization

**Repos:** doob

**Files:**
- Create: `src/commands/schema.rs`

- [ ] **Step 1: Write the failing test**

In `src/commands/schema.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_serializes_to_json() {
        let manifest = build_manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["name"], "doob");
        assert!(v["commands"].as_array().unwrap().len() > 5);
    }

    #[test]
    fn todo_list_command_present() {
        let manifest = build_manifest();
        let cmd = manifest.commands.iter().find(|c| c.name == "todo list").unwrap();
        assert!(cmd.params.iter().any(|p| p.name == "status"));
        assert!(cmd.params.iter().any(|p| p.name == "project"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cd /Users/joe/dev/doob && cargo test commands::schema
```
Expected: compile error — module not found.

- [ ] **Step 3: Write the manifest types and `build_manifest()`**

```rust
// src/commands/schema.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CliManifest {
    pub name: String,
    pub version: String,
    pub commands: Vec<CommandSchema>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandSchema {
    /// Flat command name, e.g. "todo list", "todo add", "note add"
    pub name: String,
    pub description: String,
    pub params: Vec<ParamSchema>,
    /// true if the command supports --json output
    pub json_output: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParamSchema {
    /// Snake-case param name, e.g. "status", "project"
    pub name: String,
    /// CLI flag, e.g. "--status"
    pub flag: String,
    pub required: bool,
    pub description: String,
    /// JSON Schema type: "string", "integer", "boolean", "array"
    #[serde(rename = "type")]
    pub ty: String,
}

pub fn build_manifest() -> CliManifest {
    CliManifest {
        name: "doob".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        commands: vec![
            CommandSchema {
                name: "todo add".to_string(),
                description: "Add one or more todos".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "content".to_string(), flag: "--content".to_string(), required: true, description: "Task description(s)".to_string(), ty: "array".to_string() },
                    ParamSchema { name: "priority".to_string(), flag: "--priority".to_string(), required: false, description: "Priority level".to_string(), ty: "string".to_string() },
                    ParamSchema { name: "project".to_string(), flag: "--project".to_string(), required: false, description: "Project name".to_string(), ty: "string".to_string() },
                    ParamSchema { name: "tags".to_string(), flag: "--tags".to_string(), required: false, description: "Comma-separated tags".to_string(), ty: "string".to_string() },
                    ParamSchema { name: "blocks".to_string(), flag: "--blocks".to_string(), required: false, description: "UUIDs this todo blocks".to_string(), ty: "string".to_string() },
                    ParamSchema { name: "blocked_by".to_string(), flag: "--blocked-by".to_string(), required: false, description: "UUIDs that block this todo".to_string(), ty: "string".to_string() },
                ],
            },
            CommandSchema {
                name: "todo list".to_string(),
                description: "List todos".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "status".to_string(), flag: "--status".to_string(), required: false, description: "Filter by status".to_string(), ty: "string".to_string() },
                    ParamSchema { name: "project".to_string(), flag: "--project".to_string(), required: false, description: "Filter by project".to_string(), ty: "string".to_string() },
                    ParamSchema { name: "limit".to_string(), flag: "--limit".to_string(), required: false, description: "Max results".to_string(), ty: "integer".to_string() },
                ],
            },
            CommandSchema {
                name: "todo complete".to_string(),
                description: "Complete one or more todos".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "ids".to_string(), flag: "--ids".to_string(), required: true, description: "Todo ID(s)".to_string(), ty: "array".to_string() },
                ],
            },
            CommandSchema {
                name: "todo undo".to_string(),
                description: "Undo completion — mark todos as pending".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "ids".to_string(), flag: "--ids".to_string(), required: true, description: "Todo ID(s)".to_string(), ty: "array".to_string() },
                ],
            },
            CommandSchema {
                name: "todo remove".to_string(),
                description: "Remove todos".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "ids".to_string(), flag: "--ids".to_string(), required: true, description: "Todo ID(s)".to_string(), ty: "array".to_string() },
                ],
            },
            CommandSchema {
                name: "todo due".to_string(),
                description: "Set or clear due date for a todo".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "id".to_string(), flag: "--id".to_string(), required: true, description: "Todo ID".to_string(), ty: "string".to_string() },
                    ParamSchema { name: "date".to_string(), flag: "--date".to_string(), required: false, description: "Due date (YYYY-MM-DD or 'clear')".to_string(), ty: "string".to_string() },
                ],
            },
            CommandSchema {
                name: "todo deps".to_string(),
                description: "Show dependency chain for a todo".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "id".to_string(), flag: "--id".to_string(), required: true, description: "Todo UUID".to_string(), ty: "string".to_string() },
                ],
            },
            CommandSchema {
                name: "note add".to_string(),
                description: "Add one or more notes".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "content".to_string(), flag: "--content".to_string(), required: true, description: "Note content".to_string(), ty: "array".to_string() },
                    ParamSchema { name: "project".to_string(), flag: "--project".to_string(), required: false, description: "Project name".to_string(), ty: "string".to_string() },
                    ParamSchema { name: "tags".to_string(), flag: "--tags".to_string(), required: false, description: "Comma-separated tags".to_string(), ty: "string".to_string() },
                ],
            },
            CommandSchema {
                name: "note list".to_string(),
                description: "List notes".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "project".to_string(), flag: "--project".to_string(), required: false, description: "Filter by project".to_string(), ty: "string".to_string() },
                ],
            },
            CommandSchema {
                name: "note remove".to_string(),
                description: "Remove notes".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "ids".to_string(), flag: "--ids".to_string(), required: true, description: "Note ID(s)".to_string(), ty: "array".to_string() },
                ],
            },
            CommandSchema {
                name: "search".to_string(),
                description: "Full-text search across todos and notes".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "query".to_string(), flag: "--query".to_string(), required: true, description: "Search query".to_string(), ty: "string".to_string() },
                    ParamSchema { name: "type".to_string(), flag: "--type".to_string(), required: false, description: "Filter by type: todo, note, or all".to_string(), ty: "string".to_string() },
                    ParamSchema { name: "project".to_string(), flag: "--project".to_string(), required: false, description: "Filter by project".to_string(), ty: "string".to_string() },
                ],
            },
            CommandSchema {
                name: "stats".to_string(),
                description: "Analytics and statistics".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "project".to_string(), flag: "--project".to_string(), required: false, description: "Filter by project".to_string(), ty: "string".to_string() },
                    ParamSchema { name: "window".to_string(), flag: "--window".to_string(), required: false, description: "Time window in days".to_string(), ty: "integer".to_string() },
                ],
            },
            CommandSchema {
                name: "handoff list".to_string(),
                description: "List handoff items".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "project".to_string(), flag: "--project".to_string(), required: false, description: "Filter by project".to_string(), ty: "string".to_string() },
                    ParamSchema { name: "status".to_string(), flag: "--status".to_string(), required: false, description: "Filter by status".to_string(), ty: "string".to_string() },
                ],
            },
            CommandSchema {
                name: "handoff sync".to_string(),
                description: "Bidirectional sync with HANDOFF.yaml".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "file".to_string(), flag: "--file".to_string(), required: true, description: "Path to HANDOFF.yaml".to_string(), ty: "string".to_string() },
                ],
            },
            CommandSchema {
                name: "archive list".to_string(),
                description: "List archived todos".to_string(),
                json_output: true,
                params: vec![
                    ParamSchema { name: "project".to_string(), flag: "--project".to_string(), required: false, description: "Filter by project".to_string(), ty: "string".to_string() },
                ],
            },
        ],
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

```
cd /Users/joe/dev/doob && cargo test commands::schema
```
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```
cd /Users/joe/dev/doob
git add src/commands/schema.rs
git commit -m "feat(schema): add schema manifest types and build_manifest()"
```

---

## Task 2: Wire `doob schema` into the CLI

**Repos:** doob

**Files:**
- Modify: `src/commands/mod.rs`
- Modify: `src/cli.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write the failing test**

In `src/commands/schema.rs` tests block, add:

```rust
#[test]
fn schema_command_outputs_valid_json() {
    // Smoke test: build_manifest() -> serialize -> deserialize -> name check
    let manifest = build_manifest();
    let json = serde_json::to_string_pretty(&manifest).unwrap();
    let back: CliManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, "doob");
    assert!(!back.version.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cd /Users/joe/dev/doob && cargo test schema_command_outputs_valid_json
```
Expected: compile error — `commands::schema` not in `mod.rs`.

- [ ] **Step 3: Expose module in `src/commands/mod.rs`**

Add to `src/commands/mod.rs`:
```rust
pub mod schema;
```

- [ ] **Step 4: Add `Schema` variant to `Commands` in `src/cli.rs`**

In the `Commands` enum in `src/cli.rs`, add:

```rust
/// Print machine-readable JSON manifest of all commands and params
Schema,
```

- [ ] **Step 5: Handle `Commands::Schema` in `src/main.rs`**

In the `match cli.command` block in `src/main.rs`, add:

```rust
Commands::Schema => {
    let manifest = doob::commands::schema::build_manifest();
    println!("{}", serde_json::to_string_pretty(&manifest)?);
    Ok(())
}
```

- [ ] **Step 6: Run tests and verify**

```
cd /Users/joe/dev/doob && cargo test && cargo clippy
```
Expected: all tests pass, no clippy warnings.

- [ ] **Step 7: Smoke test the command**

```
cd /Users/joe/dev/doob && cargo run -- schema | head -20
```
Expected: JSON with `"name": "doob"` and a `"commands"` array.

- [ ] **Step 8: Commit**

```
cd /Users/joe/dev/doob
git add src/commands/mod.rs src/commands/schema.rs src/cli.rs src/main.rs
git commit -m "feat: add doob schema command — emits CLI manifest as JSON"
```

---

## Task 3: `BackendKind::Cli` variant in mcpipe discovery

**Repos:** mcpipe

**Files:**
- Modify: `src/discovery.rs`

- [ ] **Step 1: Write the failing test**

Add to the test block in `src/discovery.rs`:

```rust
#[test]
fn cli_backend_kind_into_backend() {
    let source = DiscoveredSource {
        name: "doob".to_string(),
        kind: BackendKind::Cli { command: "doob".to_string() },
        origin: "manual".to_string(),
    };
    // Should not panic — CliBackend::new() is cheap
    let _backend = source.into_backend();
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cd /Users/joe/dev/mcpipe && cargo test discovery
```
Expected: compile error — `BackendKind::Cli` does not exist.

- [ ] **Step 3: Add the variant**

In `src/discovery.rs`, add to `BackendKind`:

```rust
/// CLI tool exposing a `schema` subcommand.
Cli { command: String },
```

In `into_backend()`, add the match arm:

```rust
BackendKind::Cli { command } => {
    use crate::backend::cli::CliBackend;
    Box::new(CliBackend::new(command))
}
```

- [ ] **Step 4: Run test — expect compile error on missing CliBackend**

```
cd /Users/joe/dev/mcpipe && cargo test discovery
```
Expected: compile error — `crate::backend::cli` not found. That's correct; we implement it next.

- [ ] **Step 5: Stub `src/backend/cli.rs` to unblock compilation**

```rust
// src/backend/cli.rs
use async_trait::async_trait;
use crate::backend::Backend;
use crate::domain::{ArgMap, BackendError, CommandDef};

pub struct CliBackend {
    command: String,
}

impl CliBackend {
    pub fn new(command: impl Into<String>) -> Self {
        Self { command: command.into() }
    }
}

#[async_trait]
impl Backend for CliBackend {
    async fn discover(&self) -> Result<Vec<CommandDef>, BackendError> {
        unimplemented!()
    }
    async fn execute(&self, _cmd: &CommandDef, _args: ArgMap) -> Result<serde_json::Value, BackendError> {
        unimplemented!()
    }
}
```

- [ ] **Step 6: Add module to `src/backend/mod.rs`**

```rust
pub mod cli;
```

- [ ] **Step 7: Run tests to verify compile + test pass**

```
cd /Users/joe/dev/mcpipe && cargo test discovery
```
Expected: 1 test passes.

- [ ] **Step 8: Commit**

```
cd /Users/joe/dev/mcpipe
git add src/discovery.rs src/backend/cli.rs src/backend/mod.rs
git commit -m "feat(discovery): add BackendKind::Cli variant and CliBackend stub"
```

---

## Task 4: `CliBackend::discover()` — parse schema manifest

**Repos:** mcpipe

**Files:**
- Modify: `src/backend/cli.rs`

- [ ] **Step 1: Write the failing test**

Add to `src/backend/cli.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Uses the real `doob` binary on PATH — run only with --features integration
    #[cfg(feature = "integration")]
    #[tokio::test]
    async fn discover_doob_commands() {
        let backend = CliBackend::new("doob");
        let cmds = backend.discover().await.unwrap();
        assert!(cmds.len() > 5);
        assert!(cmds.iter().any(|c| c.name == "todo-list"));
        assert!(cmds.iter().any(|c| c.name == "todo-add"));
        assert!(cmds.iter().any(|c| c.name == "search"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cd /Users/joe/dev/mcpipe && cargo test --features integration cli_backend
```
Expected: fails — `discover()` panics with `unimplemented!()`.

- [ ] **Step 3: Add manifest deserialization types**

Add to `src/backend/cli.rs` above `CliBackend`:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CliManifest {
    commands: Vec<ManifestCommand>,
}

#[derive(Debug, Deserialize)]
struct ManifestCommand {
    name: String,
    description: String,
    params: Vec<ManifestParam>,
}

#[derive(Debug, Deserialize)]
struct ManifestParam {
    name: String,
    flag: String,
    required: bool,
    description: String,
    #[serde(rename = "type")]
    ty: String,
}
```

- [ ] **Step 4: Implement `discover()`**

Replace the `unimplemented!()` in `discover()`:

```rust
async fn discover(&self) -> Result<Vec<CommandDef>, BackendError> {
    use tokio::process::Command;
    use crate::domain::{ParamDef, ParamLocation};

    let output = Command::new(&self.command)
        .args(["schema", "--json"])
        .output()
        .await
        .map_err(|e| BackendError::Discovery(format!("failed to run `{}`: {}", self.command, e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BackendError::Discovery(format!(
            "`{} schema` exited non-zero: {}", self.command, stderr
        )));
    }

    let manifest: CliManifest = serde_json::from_slice(&output.stdout)
        .map_err(|e| BackendError::Schema(format!("invalid schema JSON: {e}")))?;

    let cmds = manifest.commands.into_iter().map(|mc| {
        // "todo list" -> "todo-list" (valid CLI subcommand name)
        let name = mc.name.replace(' ', "-");
        let params = mc.params.into_iter().map(|p| ParamDef {
            name: p.name.clone(),
            original_name: p.flag.trim_start_matches('-').to_string(),
            required: p.required,
            description: p.description,
            location: ParamLocation::ToolInput,
            schema: type_str_to_schema(&p.ty),
        }).collect();
        CommandDef { name, description: mc.description, params, source_name: self.command.clone() }
    }).collect();

    Ok(cmds)
}
```

Add the helper after `CliBackend`'s `impl Backend`:

```rust
fn type_str_to_schema(ty: &str) -> serde_json::Value {
    match ty {
        "integer" => serde_json::json!({"type": "integer"}),
        "boolean" => serde_json::json!({"type": "boolean"}),
        "array"   => serde_json::json!({"type": "array", "items": {"type": "string"}}),
        _         => serde_json::json!({"type": "string"}),
    }
}
```

- [ ] **Step 5: Run tests**

```
cd /Users/joe/dev/mcpipe && cargo test --features integration cli_backend
```
Expected: `discover_doob_commands` passes.

- [ ] **Step 6: Commit**

```
cd /Users/joe/dev/mcpipe
git add src/backend/cli.rs
git commit -m "feat(cli-backend): implement discover() via doob schema --json"
```

---

## Task 5: `CliBackend::execute()` — run subcommand as subprocess

**Repos:** mcpipe

**Files:**
- Modify: `src/backend/cli.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` block in `src/backend/cli.rs`:

```rust
#[cfg(feature = "integration")]
#[tokio::test]
async fn execute_doob_todo_list() {
    let backend = CliBackend::new("doob");
    let cmds = backend.discover().await.unwrap();
    let list_cmd = cmds.iter().find(|c| c.name == "todo-list").unwrap().clone();
    let result = backend.execute(&list_cmd, std::collections::HashMap::new()).await.unwrap();
    // Result is the parsed JSON from doob todo list --json
    assert!(result.get("todos").is_some() || result.get("count").is_some());
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cd /Users/joe/dev/mcpipe && cargo test --features integration execute_doob
```
Expected: fails — `execute()` panics.

- [ ] **Step 3: Implement `execute()`**

Replace `unimplemented!()` in `execute()`:

```rust
async fn execute(&self, cmd: &CommandDef, args: ArgMap) -> Result<serde_json::Value, BackendError> {
    use tokio::process::Command;

    // "todo-list" -> ["todo", "list"]
    let parts: Vec<&str> = cmd.name.splitn(2, '-').collect();

    let mut argv: Vec<String> = parts.iter().map(|s| s.to_string()).collect();
    argv.push("--json".to_string());

    // Append provided args as flags
    for param in &cmd.params {
        let key = &param.name;
        if let Some(val) = args.get(key) {
            // Map param name back to CLI flag via original_name
            let flag = format!("--{}", param.original_name.replace('_', "-"));
            match val {
                serde_json::Value::Array(items) => {
                    // Positional or repeated: pass each as separate arg
                    for item in items {
                        let s = match item {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        argv.push(flag.clone());
                        argv.push(s);
                    }
                }
                serde_json::Value::Null => {}
                other => {
                    argv.push(flag);
                    argv.push(match other {
                        serde_json::Value::String(s) => s.clone(),
                        n => n.to_string(),
                    });
                }
            }
        }
    }

    let output = Command::new(&self.command)
        .args(&argv)
        .output()
        .await
        .map_err(|e| BackendError::Transport(format!("spawn error: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BackendError::Execution(format!(
            "`{} {}` failed: {}", self.command, argv.join(" "), stderr
        )));
    }

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| BackendError::Execution(format!("JSON parse error: {e}")))?;

    Ok(value)
}
```

- [ ] **Step 4: Run tests**

```
cd /Users/joe/dev/mcpipe && cargo test --features integration
```
Expected: both `discover_doob_commands` and `execute_doob_todo_list` pass.

- [ ] **Step 5: Commit**

```
cd /Users/joe/dev/mcpipe
git add src/backend/cli.rs
git commit -m "feat(cli-backend): implement execute() — dispatches subcommand as subprocess"
```

---

## Task 6: OpenAPI 3.1 generator

**Repos:** mcpipe

**Files:**
- Create: `src/openapi_gen.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `src/openapi_gen.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CommandDef, ParamDef, ParamLocation};

    fn sample_commands() -> Vec<CommandDef> {
        vec![
            CommandDef {
                name: "todo-list".to_string(),
                description: "List todos".to_string(),
                source_name: "doob".to_string(),
                params: vec![
                    ParamDef {
                        name: "status".to_string(),
                        original_name: "status".to_string(),
                        required: false,
                        description: "Filter by status".to_string(),
                        location: ParamLocation::ToolInput,
                        schema: serde_json::json!({"type": "string"}),
                    },
                ],
            },
            CommandDef {
                name: "todo-add".to_string(),
                description: "Add todos".to_string(),
                source_name: "doob".to_string(),
                params: vec![
                    ParamDef {
                        name: "content".to_string(),
                        original_name: "content".to_string(),
                        required: true,
                        description: "Task description".to_string(),
                        location: ParamLocation::ToolInput,
                        schema: serde_json::json!({"type": "string"}),
                    },
                ],
            },
        ]
    }

    #[test]
    fn generates_valid_openapi_document() {
        let doc = generate("doob", "0.1.0", &sample_commands());
        assert_eq!(doc["openapi"], "3.1.0");
        assert_eq!(doc["info"]["title"], "doob");
        assert!(doc["paths"].is_object());
    }

    #[test]
    fn get_command_maps_to_get_path() {
        let doc = generate("doob", "0.1.0", &sample_commands());
        let path = &doc["paths"]["/todo-list"];
        assert!(path["get"].is_object(), "todo-list should be GET");
        let params = path["get"]["parameters"].as_array().unwrap();
        assert!(params.iter().any(|p| p["name"] == "status"));
    }

    #[test]
    fn post_command_maps_to_post_path() {
        let doc = generate("doob", "0.1.0", &sample_commands());
        let path = &doc["paths"]["/todo-add"];
        assert!(path["post"].is_object(), "todo-add (has required param) should be POST");
        let body = &path["post"]["requestBody"]["content"]["application/json"]["schema"];
        assert!(body["properties"]["content"].is_object());
    }

    #[test]
    fn yaml_output_is_valid() {
        let doc = generate("doob", "0.1.0", &sample_commands());
        let yaml = to_yaml(&doc).unwrap();
        assert!(yaml.contains("openapi: 3.1.0"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cd /Users/joe/dev/mcpipe && cargo test openapi_gen
```
Expected: compile error — module not found.

- [ ] **Step 3: Implement the generator**

```rust
// src/openapi_gen.rs
use crate::domain::CommandDef;

/// Generate an OpenAPI 3.1 document from a list of CommandDefs.
/// Commands with any required param become POST (request body).
/// Commands with only optional params become GET (query params).
pub fn generate(tool_name: &str, version: &str, commands: &[CommandDef]) -> serde_json::Value {
    let mut paths = serde_json::Map::new();

    for cmd in commands {
        let has_required = cmd.params.iter().any(|p| p.required);
        let path_key = format!("/{}", cmd.name);

        let operation = if has_required {
            build_post_operation(cmd)
        } else {
            build_get_operation(cmd)
        };

        let method = if has_required { "post" } else { "get" };
        let mut path_item = serde_json::Map::new();
        path_item.insert(method.to_string(), operation);
        paths.insert(path_key, serde_json::Value::Object(path_item));
    }

    serde_json::json!({
        "openapi": "3.1.0",
        "info": {
            "title": tool_name,
            "version": version,
        },
        "paths": paths,
    })
}

fn build_get_operation(cmd: &CommandDef) -> serde_json::Value {
    let parameters: Vec<serde_json::Value> = cmd.params.iter().map(|p| {
        serde_json::json!({
            "name": p.name,
            "in": "query",
            "required": p.required,
            "description": p.description,
            "schema": p.schema,
        })
    }).collect();

    serde_json::json!({
        "summary": cmd.description,
        "operationId": cmd.name,
        "parameters": parameters,
        "responses": {
            "200": {
                "description": "Success",
                "content": {
                    "application/json": {
                        "schema": { "type": "object", "additionalProperties": true }
                    }
                }
            }
        }
    })
}

fn build_post_operation(cmd: &CommandDef) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let mut required_fields: Vec<serde_json::Value> = vec![];

    for p in &cmd.params {
        properties.insert(p.name.clone(), serde_json::json!({
            "description": p.description,
            "schema": p.schema,
        }));
        if p.required {
            required_fields.push(serde_json::Value::String(p.name.clone()));
        }
    }

    let body_schema = if required_fields.is_empty() {
        serde_json::json!({ "type": "object", "properties": properties })
    } else {
        serde_json::json!({ "type": "object", "properties": properties, "required": required_fields })
    };

    serde_json::json!({
        "summary": cmd.description,
        "operationId": cmd.name,
        "requestBody": {
            "required": true,
            "content": {
                "application/json": { "schema": body_schema }
            }
        },
        "responses": {
            "200": {
                "description": "Success",
                "content": {
                    "application/json": {
                        "schema": { "type": "object", "additionalProperties": true }
                    }
                }
            }
        }
    })
}

/// Serialize an OpenAPI document to YAML string.
pub fn to_yaml(doc: &serde_json::Value) -> Result<String, serde_yaml::Error> {
    serde_yaml::to_string(doc)
}
```

- [ ] **Step 4: Expose module in `src/lib.rs`**

Add to `src/lib.rs`:
```rust
pub mod openapi_gen;
```

- [ ] **Step 5: Run tests**

```
cd /Users/joe/dev/mcpipe && cargo test openapi_gen
```
Expected: 4 tests pass.

- [ ] **Step 6: Commit**

```
cd /Users/joe/dev/mcpipe
git add src/openapi_gen.rs src/lib.rs
git commit -m "feat: add openapi_gen module — generates OpenAPI 3.1 spec from CommandDefs"
```

---

## Task 7: Wire `--cli` and `--gen-openapi` flags in `main.rs`

**Repos:** mcpipe

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Write the failing test**

Add to `tests/cli_backend.rs` (create file):

```rust
// tests/cli_backend.rs
#[cfg(feature = "integration")]
mod tests {
    #[tokio::test]
    async fn mcpipe_cli_list_doob() {
        // Verify CliBackend round-trips through main discovery path
        use mcpipe::backend::cli::CliBackend;
        use mcpipe::backend::Backend;
        let backend = CliBackend::new("doob");
        let cmds = backend.discover().await.unwrap();
        assert!(cmds.iter().any(|c| c.name == "todo-list"));
    }

    #[tokio::test]
    async fn gen_openapi_from_doob() {
        use mcpipe::backend::cli::CliBackend;
        use mcpipe::backend::Backend;
        use mcpipe::openapi_gen;
        let backend = CliBackend::new("doob");
        let cmds = backend.discover().await.unwrap();
        let doc = openapi_gen::generate("doob", "0.1.0", &cmds);
        let yaml = openapi_gen::to_yaml(&doc).unwrap();
        assert!(yaml.contains("openapi: 3.1.0"));
        assert!(yaml.contains("/todo-list"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cd /Users/joe/dev/mcpipe && cargo test --features integration --test cli_backend
```
Expected: `gen_openapi_from_doob` fails if `openapi_gen` not yet pub, or test file compiles and passes (both integration tasks already done). Fix any compile errors.

- [ ] **Step 3: Add `--cli` and `--gen-openapi` flags to `build_global_parser()` in `main.rs`**

In `build_global_parser()` in `src/main.rs`, add these args alongside the existing `--mcp`, `--spec`, etc.:

```rust
.arg(
    Arg::new("cli")
        .long("cli")
        .value_name("COMMAND")
        .help("CLI tool exposing a `schema` subcommand (e.g. doob)")
        .num_args(1),
)
.arg(
    Arg::new("gen-openapi")
        .long("gen-openapi")
        .action(ArgAction::SetTrue)
        .help("Generate OpenAPI 3.1 spec from discovered commands and print to stdout"),
)
.arg(
    Arg::new("openapi-output")
        .long("openapi-output")
        .value_name("FILE")
        .help("Write generated OpenAPI spec to FILE instead of stdout")
        .num_args(1),
)
```

- [ ] **Step 4: Handle `--cli` backend construction in `run()`**

In the backend construction block in `run()`, after the `} else if let Some(spec) = ...` branch, add:

```rust
} else if let Some(cli_cmd) = matches.get_one::<String>("cli") {
    use mcpipe::backend::cli::CliBackend;
    Box::new(CliBackend::new(cli_cmd.clone()))
```

- [ ] **Step 5: Handle `--gen-openapi` after discovery**

After the `list_only` block in `run()`, add handling for `--gen-openapi`. Find the block that runs `backend.discover()` (the `list_only` branch) and add, in the same region:

```rust
let gen_openapi = matches.get_flag("gen-openapi");
let openapi_output = matches.get_one::<String>("openapi-output").cloned();

if gen_openapi {
    let commands = backend.discover().await
        .context("discovering commands for OpenAPI generation")?;
    let tool_name = matches.get_one::<String>("cli")
        .map(|s| s.as_str())
        .unwrap_or("api");
    let doc = mcpipe::openapi_gen::generate(tool_name, "0.1.0", &commands);
    let yaml = mcpipe::openapi_gen::to_yaml(&doc)
        .context("serializing OpenAPI spec to YAML")?;
    if let Some(path) = openapi_output {
        std::fs::write(&path, &yaml)
            .with_context(|| format!("writing OpenAPI spec to {path}"))?;
        eprintln!("Wrote OpenAPI spec to {path}");
    } else {
        print!("{yaml}");
    }
    return Ok(());
}
```

- [ ] **Step 6: Run all tests**

```
cd /Users/joe/dev/mcpipe && cargo test && cargo test --features integration && cargo clippy
```
Expected: all pass, no warnings.

- [ ] **Step 7: Smoke test end-to-end**

```
cd /Users/joe/dev/mcpipe && cargo build --release
./target/release/mcpipe --cli doob --list
./target/release/mcpipe --cli doob --gen-openapi
```
Expected: first command lists doob commands; second prints valid YAML OpenAPI spec.

- [ ] **Step 8: Commit**

```
cd /Users/joe/dev/mcpipe
git add src/main.rs tests/cli_backend.rs
git commit -m "feat: wire --cli and --gen-openapi flags for CliBackend + OpenAPI generation"
```

---

---

## Future: BAML Integration (separate plan)

Once this plan is complete and `mcpipe --cli doob --gen-openapi` produces a verified spec:

1. Run `mcpipe --cli doob --gen-openapi --openapi-output doob.openapi.yaml`
2. Use that spec as the source of truth to write `devloop/crates/baml/baml_src/doob.baml` —
   one BAML `class` per response shape, one `function` per operation, following the pattern
   in `maestro.baml`
3. Run `baml generate` to produce the Rust client
4. Any tool that gets a `schema` subcommand (obfsck, devkit, minibox) follows the same path

That plan lives in the devloop repo, not here.

---

## Self-Review

**Spec coverage:**
- `doob schema` command → Tasks 1–2 ✓
- `CliBackend` implementing `Backend` trait → Tasks 3–5 ✓
- `BackendKind::Cli` in discovery → Task 3 ✓
- OpenAPI 3.1 generator → Task 6 ✓
- `--cli` / `--gen-openapi` flags → Task 7 ✓
- GET vs POST routing logic → Task 6 ✓
- YAML output → Tasks 6–7 ✓

**Placeholder scan:** None found.

**Type consistency:**
- `CliManifest` / `ManifestCommand` / `ManifestParam` defined in Task 4, used only in Task 4 ✓
- `CommandDef` / `ParamDef` / `ParamLocation` from `domain.rs` — used consistently Tasks 4–7 ✓
- `CliBackend::new(command)` defined Task 3, used Tasks 4, 5, 7 ✓
- `openapi_gen::generate(tool_name, version, &cmds)` defined Task 6, used Tasks 6, 7 ✓
- `openapi_gen::to_yaml(&doc)` defined Task 6, used Tasks 6, 7 ✓
