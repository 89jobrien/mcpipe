# mcpipe Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a standalone Rust CLI binary (`mcpipe`) that turns MCP servers, OpenAPI specs, and GraphQL endpoints into shell-callable subcommands.

**Architecture:** Hexagonal architecture — `Backend` trait is the central port; `McpBackend`, `OpenApiBackend`, `GraphQlBackend` are adapters. `main.rs` is the composition root holding `Box<dyn Backend>`. CLI subcommands are generated dynamically at runtime from `discover()` output.

**Tech Stack:** Rust 2024 edition, tokio, clap 4, reqwest 0.12, serde_json, async-trait, anyhow, sha2, eventsource-client.

---

## File Map

| File                                | Responsibility                                                                                                      |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `Cargo.toml`                        | workspace manifest, all deps                                                                                        |
| `src/main.rs`                       | composition root: parse global flags, build `Box<dyn Backend>`, orchestrate discover → cli → execute → format       |
| `src/domain.rs`                     | `CommandDef`, `ParamDef`, `ParamLocation`, `ArgMap`, `BackendError` — zero external deps (except serde_json)        |
| `src/backend/mod.rs`                | `Backend` async trait                                                                                               |
| `src/backend/mcp.rs`                | `McpBackend` — stdio and HTTP/SSE transports                                                                        |
| `src/backend/openapi.rs`            | `OpenApiBackend` — spec fetch, `$ref` resolve, operation → `CommandDef`                                             |
| `src/backend/graphql.rs`            | `GraphQlBackend` — introspection → `CommandDef`, execute mutation/query                                             |
| `src/cli.rs`                        | `build_command()` — walk `Vec<CommandDef>`, return `clap::Command` tree; `extract_args()` — matched args → `ArgMap` |
| `src/cache.rs`                      | `Cache` struct — load/save `Vec<CommandDef>` with TTL, SHA-256 key                                                  |
| `src/format.rs`                     | `output()` — pretty/raw/jq/head formatting of `serde_json::Value`                                                   |
| `src/secret.rs`                     | `resolve_secret()` — `env:` / `file:` / literal                                                                     |
| `tests/openapi_adapter.rs`          | fixture-based adapter tests for `OpenApiBackend`                                                                    |
| `tests/graphql_adapter.rs`          | fixture-based adapter tests for `GraphQlBackend`                                                                    |
| `tests/mcp_adapter.rs`              | process-spawn adapter tests for `McpBackend` stdio                                                                  |
| `tests/fixtures/petstore.json`      | petstore-style OpenAPI fixture                                                                                      |
| `tests/fixtures/introspection.json` | GraphQL introspection fixture                                                                                       |
| `tests/fixtures/mcp_echo.py`        | minimal MCP stdio echo server (Python, used as test subprocess)                                                     |

---

## Task 1: Scaffold project

**Files:**

- Create: `Cargo.toml`
- Create: `src/main.rs`

- [ ] **Step 1: Init cargo project**

```bash
cd /Users/joe/dev/mcpipe
cargo init --name mcpipe
```

Expected: `src/main.rs` with `fn main() {}`, `Cargo.toml` created.

- [ ] **Step 2: Replace Cargo.toml with full deps**

```toml
[package]
name = "mcpipe"
version = "0.1.0"
edition = "2024"
description = "Turn any MCP server, OpenAPI spec, or GraphQL endpoint into a shell CLI"
license = "MIT OR Apache-2.0"

[[bin]]
name = "mcpipe"
path = "src/main.rs"

[features]
integration = []

[dependencies]
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "stream"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
async-trait = "0.1"
anyhow = "1"
sha2 = { version = "0.10", features = [] }
hex = "0.4"
eventsource-client = "0.12"
```

- [ ] **Step 3: Verify it compiles**

```bash
cd /Users/joe/dev/mcpipe
cargo check
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
cd /Users/joe/dev/mcpipe
git add Cargo.toml src/main.rs
git commit -m "chore: scaffold mcpipe cargo project"
```

---

## Task 2: Domain types

**Files:**

- Create: `src/domain.rs`
- Modify: `src/main.rs` (add `mod domain;`)

- [ ] **Step 1: Write failing test for `CommandDef` construction**

Add to `src/domain.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandDef {
    pub name: String,
    pub description: String,
    pub params: Vec<ParamDef>,
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParamDef {
    pub name: String,
    pub original_name: String,
    pub required: bool,
    pub description: String,
    pub location: ParamLocation,
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ParamLocation {
    Body,
    Query,
    Path,
    Header,
    ToolInput,
}

pub type ArgMap = HashMap<String, serde_json::Value>;

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("discovery failed: {0}")]
    Discovery(String),
    #[error("execution failed: {0}")]
    Execution(String),
    #[error("command not found: {0}")]
    NotFound(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("schema error: {0}")]
    Schema(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_def_roundtrip_json() {
        let cmd = CommandDef {
            name: "list-pets".to_string(),
            description: "List all pets".to_string(),
            source_name: "listPets".to_string(),
            params: vec![ParamDef {
                name: "limit".to_string(),
                original_name: "limit".to_string(),
                required: false,
                description: "max records".to_string(),
                location: ParamLocation::Query,
                schema: serde_json::json!({"type": "integer"}),
            }],
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let back: CommandDef = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, back);
    }
}
```

- [ ] **Step 2: Add `thiserror` dep and `mod domain;` to main**

Add to `Cargo.toml` `[dependencies]`:

```toml
thiserror = "1"
```

Add to `src/main.rs`:

```rust
mod domain;

fn main() {}
```

- [ ] **Step 3: Run test**

```bash
cd /Users/joe/dev/mcpipe
cargo test domain
```

Expected: `test domain::tests::command_def_roundtrip_json ... ok`

- [ ] **Step 4: Commit**

```bash
git add src/domain.rs src/main.rs Cargo.toml
git commit -m "feat: add domain types (CommandDef, ParamDef, BackendError)"
```

---

## Task 3: Backend trait

**Files:**

- Create: `src/backend/mod.rs`
- Modify: `src/main.rs` (add `mod backend;`)

- [ ] **Step 1: Write the Backend trait**

Create `src/backend/mod.rs`:

```rust
pub mod mcp;
pub mod openapi;
pub mod graphql;

use async_trait::async_trait;
use crate::domain::{ArgMap, BackendError, CommandDef};

#[async_trait]
pub trait Backend: Send + Sync {
    async fn discover(&self) -> Result<Vec<CommandDef>, BackendError>;
    async fn execute(
        &self,
        cmd: &CommandDef,
        args: ArgMap,
    ) -> Result<serde_json::Value, BackendError>;
}
```

Create empty stub files so it compiles:

`src/backend/mcp.rs`:

```rust
// MCP backend — stdio and HTTP/SSE
```

`src/backend/openapi.rs`:

```rust
// OpenAPI backend
```

`src/backend/graphql.rs`:

```rust
// GraphQL backend
```

Add to `src/main.rs`:

```rust
mod domain;
mod backend;

fn main() {}
```

- [ ] **Step 2: Verify compiles**

```bash
cd /Users/joe/dev/mcpipe
cargo check
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/backend/ src/main.rs
git commit -m "feat: add Backend trait (port)"
```

---

## Task 4: Secret resolution

**Files:**

- Create: `src/secret.rs`
- Modify: `src/main.rs` (add `mod secret;`)

- [ ] **Step 1: Write failing tests**

Create `src/secret.rs`:

```rust
use anyhow::{bail, Context, Result};
use std::path::Path;

/// Resolve a secret value.
/// - `env:VAR` → read from environment variable
/// - `file:/path` → read file, strip trailing newline
/// - anything else → return as-is
pub fn resolve_secret(value: &str) -> Result<String> {
    if let Some(var) = value.strip_prefix("env:") {
        std::env::var(var).with_context(|| format!("env var {var:?} is not set"))
    } else if let Some(path) = value.strip_prefix("file:") {
        let content = std::fs::read_to_string(Path::new(path))
            .with_context(|| format!("reading secret file {path:?}"))?;
        Ok(content.trim_end_matches('\n').to_string())
    } else {
        Ok(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_passthrough() {
        assert_eq!(resolve_secret("mytoken").unwrap(), "mytoken");
    }

    #[test]
    fn env_prefix() {
        unsafe { std::env::set_var("MCPIPE_TEST_SECRET", "secret123") };
        assert_eq!(resolve_secret("env:MCPIPE_TEST_SECRET").unwrap(), "secret123");
    }

    #[test]
    fn env_missing_errors() {
        unsafe { std::env::remove_var("MCPIPE_TEST_MISSING") };
        assert!(resolve_secret("env:MCPIPE_TEST_MISSING").is_err());
    }

    #[test]
    fn file_prefix(tmp_dir: ()) {
        // use a temp file
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.txt");
        std::fs::write(&path, "filetoken\n").unwrap();
        let spec = format!("file:{}", path.display());
        assert_eq!(resolve_secret(&spec).unwrap(), "filetoken");
    }
}
```

- [ ] **Step 2: Add `tempfile` dev-dep**

```toml
[dev-dependencies]
tempfile = "3"
```

Fix the test — `file_prefix` has a stray param, replace with:

```rust
#[test]
fn file_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secret.txt");
    std::fs::write(&path, "filetoken\n").unwrap();
    let spec = format!("file:{}", path.display());
    assert_eq!(resolve_secret(&spec).unwrap(), "filetoken");
}
```

Add to `src/main.rs`:

```rust
mod domain;
mod backend;
mod secret;

fn main() {}
```

- [ ] **Step 3: Run tests**

```bash
cd /Users/joe/dev/mcpipe
cargo test secret
```

Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/secret.rs src/main.rs Cargo.toml
git commit -m "feat: add secret resolution (env:/file:/literal)"
```

---

## Task 5: Output formatting

**Files:**

- Create: `src/format.rs`
- Modify: `src/main.rs` (add `mod format;`)

- [ ] **Step 1: Write failing tests**

Create `src/format.rs`:

```rust
use anyhow::{bail, Result};
use std::process::{Command, Stdio};

pub struct FormatOptions {
    pub pretty: bool,
    pub raw: bool,
    pub jq: Option<String>,
    pub head: Option<usize>,
}

/// Format a JSON value to a String per options.
pub fn format_value(value: &serde_json::Value, opts: &FormatOptions) -> Result<String> {
    let value = if let Some(n) = opts.head {
        match value {
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().take(n).cloned().collect())
            }
            other => other.clone(),
        }
    } else {
        value.clone()
    };

    if opts.raw {
        return Ok(match &value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        });
    }

    let json_str = if opts.pretty {
        serde_json::to_string_pretty(&value)?
    } else {
        serde_json::to_string(&value)?
    };

    if let Some(expr) = &opts.jq {
        return run_jq(&json_str, expr);
    }

    Ok(json_str)
}

fn run_jq(json: &str, expr: &str) -> Result<String> {
    let mut child = Command::new("jq")
        .arg(expr)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("jq not found: {e}"))?;

    use std::io::Write;
    child.stdin.as_mut().unwrap().write_all(json.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!("jq error: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(String::from_utf8(out.stdout)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn opts() -> FormatOptions {
        FormatOptions { pretty: false, raw: false, jq: None, head: None }
    }

    #[test]
    fn compact_by_default() {
        let v = json!({"a": 1});
        let out = format_value(&v, &opts()).unwrap();
        assert_eq!(out, r#"{"a":1}"#);
    }

    #[test]
    fn pretty_flag() {
        let v = json!({"a": 1});
        let out = format_value(&v, &FormatOptions { pretty: true, ..opts() }).unwrap();
        assert!(out.contains('\n'));
    }

    #[test]
    fn raw_string() {
        let v = json!("hello");
        let out = format_value(&v, &FormatOptions { raw: true, ..opts() }).unwrap();
        assert_eq!(out, "hello");
    }

    #[test]
    fn head_truncates_array() {
        let v = json!([1, 2, 3, 4, 5]);
        let out = format_value(&v, &FormatOptions { head: Some(3), ..opts() }).unwrap();
        let back: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(back, json!([1, 2, 3]));
    }

    #[test]
    fn head_noop_on_object() {
        let v = json!({"a": 1});
        let out = format_value(&v, &FormatOptions { head: Some(1), ..opts() }).unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&out).unwrap(), v);
    }
}
```

Add to `src/main.rs`:

```rust
mod domain;
mod backend;
mod secret;
mod format;

fn main() {}
```

- [ ] **Step 2: Run tests**

```bash
cd /Users/joe/dev/mcpipe
cargo test format
```

Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/format.rs src/main.rs
git commit -m "feat: add output formatting (pretty/raw/head/jq)"
```

---

## Task 6: Disk cache

**Files:**

- Create: `src/cache.rs`
- Modify: `src/main.rs` (add `mod cache;`)

- [ ] **Step 1: Write failing tests**

Create `src/cache.rs`:

```rust
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::domain::CommandDef;

pub struct Cache {
    dir: PathBuf,
    ttl: Duration,
}

impl Cache {
    pub fn new(dir: PathBuf, ttl: Duration) -> Self {
        Self { dir, ttl }
    }

    pub fn default_dir() -> PathBuf {
        let base = std::env::var("MCPIPE_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::cache_dir()
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join("mcpipe")
            });
        base
    }

    fn key(source: &str) -> String {
        let hash = Sha256::digest(source.as_bytes());
        hex::encode(&hash[..8]) // 16 hex chars
    }

    fn path(&self, source: &str) -> PathBuf {
        self.dir.join(format!("{}.json", Self::key(source)))
    }

    pub fn load(&self, source: &str) -> Option<Vec<CommandDef>> {
        let path = self.path(source);
        let meta = std::fs::metadata(&path).ok()?;
        let modified = meta.modified().ok()?;
        let age = SystemTime::now().duration_since(modified).ok()?;
        if age >= self.ttl {
            return None;
        }
        let data = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn save(&self, source: &str, cmds: &[CommandDef]) -> Result<()> {
        std::fs::create_dir_all(&self.dir)
            .context("creating cache dir")?;
        let path = self.path(source);
        let data = serde_json::to_string(cmds)?;
        std::fs::write(&path, data).context("writing cache file")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CommandDef, ParamLocation};

    fn sample_cmd() -> CommandDef {
        CommandDef {
            name: "list-pets".to_string(),
            description: "List pets".to_string(),
            source_name: "listPets".to_string(),
            params: vec![],
        }
    }

    #[test]
    fn cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().to_path_buf(), Duration::from_secs(3600));
        let cmds = vec![sample_cmd()];
        cache.save("http://example.com/spec", &cmds).unwrap();
        let loaded = cache.load("http://example.com/spec").unwrap();
        assert_eq!(loaded[0].name, "list-pets");
    }

    #[test]
    fn cache_miss_on_different_source() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().to_path_buf(), Duration::from_secs(3600));
        cache.save("source-a", &[sample_cmd()]).unwrap();
        assert!(cache.load("source-b").is_none());
    }

    #[test]
    fn cache_expired_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        // TTL of 0 seconds — always expired
        let cache = Cache::new(dir.path().to_path_buf(), Duration::from_secs(0));
        cache.save("source", &[sample_cmd()]).unwrap();
        // sleep 1ms so modified time is strictly in the past
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(cache.load("source").is_none());
    }
}
```

- [ ] **Step 2: Add `dirs` dep**

```toml
dirs = "5"
```

Add to `src/main.rs`:

```rust
mod domain;
mod backend;
mod secret;
mod format;
mod cache;

fn main() {}
```

- [ ] **Step 3: Run tests**

```bash
cd /Users/joe/dev/mcpipe
cargo test cache
```

Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/cache.rs src/main.rs Cargo.toml
git commit -m "feat: add TTL disk cache for discovered CommandDefs"
```

---

## Task 7: Dynamic CLI builder

**Files:**

- Create: `src/cli.rs`
- Modify: `src/main.rs` (add `mod cli;`)

- [ ] **Step 1: Write failing tests**

Create `src/cli.rs`:

```rust
use clap::{Arg, ArgMatches, Command};
use crate::domain::{ArgMap, CommandDef, ParamDef, ParamLocation};

/// Build a clap Command tree from a list of CommandDefs.
/// The returned Command has one subcommand per CommandDef.
pub fn build_command(app_name: &str, cmds: &[CommandDef]) -> Command {
    let mut app = Command::new(app_name)
        .subcommand_required(false)
        .arg_required_else_help(false);

    for cmd in cmds {
        let mut sub = Command::new(&cmd.name)
            .about(cmd.description.clone());

        for param in &cmd.params {
            let arg = build_arg(param);
            sub = sub.arg(arg);
        }

        app = app.subcommand(sub);
    }

    app
}

fn build_arg(param: &ParamDef) -> Arg {
    let schema_type = param.schema.get("type").and_then(|v| v.as_str()).unwrap_or("string");
    let is_bool = schema_type == "boolean";

    let mut arg = Arg::new(&param.name)
        .long(&param.name)
        .help(param.description.clone());

    if is_bool {
        arg = arg.action(clap::ArgAction::SetTrue);
    } else {
        arg = arg.value_name(param.name.to_uppercase().replace('-', "_"));
        if param.required {
            arg = arg.required(true);
        }
    }

    arg
}

/// Extract ArgMap from clap matches for a given CommandDef.
pub fn extract_args(matches: &ArgMatches, cmd: &CommandDef) -> ArgMap {
    let mut map = ArgMap::new();

    for param in &cmd.params {
        let schema_type = param.schema.get("type").and_then(|v| v.as_str()).unwrap_or("string");
        let is_bool = schema_type == "boolean";

        if is_bool {
            let val = matches.get_flag(&param.name);
            if val {
                map.insert(param.original_name.clone(), serde_json::Value::Bool(true));
            }
        } else if let Some(raw) = matches.get_one::<String>(&param.name) {
            let coerced = coerce(raw, &param.schema);
            map.insert(param.original_name.clone(), coerced);
        }
    }

    map
}

fn coerce(value: &str, schema: &serde_json::Value) -> serde_json::Value {
    match schema.get("type").and_then(|v| v.as_str()) {
        Some("integer") => value.parse::<i64>()
            .map(serde_json::Value::from)
            .unwrap_or_else(|_| serde_json::Value::String(value.to_string())),
        Some("number") => value.parse::<f64>()
            .map(serde_json::Value::from)
            .unwrap_or_else(|_| serde_json::Value::String(value.to_string())),
        Some("boolean") => serde_json::Value::Bool(
            matches!(value.to_lowercase().as_str(), "true" | "1" | "yes")
        ),
        Some("array") | Some("object") => serde_json::from_str(value)
            .unwrap_or_else(|_| serde_json::Value::String(value.to_string())),
        _ => serde_json::Value::String(value.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CommandDef, ParamDef, ParamLocation};

    fn make_cmd(params: Vec<ParamDef>) -> CommandDef {
        CommandDef {
            name: "list-pets".to_string(),
            description: "List pets".to_string(),
            source_name: "listPets".to_string(),
            params,
        }
    }

    fn int_param(name: &str, required: bool) -> ParamDef {
        ParamDef {
            name: name.to_string(),
            original_name: name.to_string(),
            required,
            description: String::new(),
            location: ParamLocation::Query,
            schema: serde_json::json!({"type": "integer"}),
        }
    }

    fn str_param(name: &str, required: bool) -> ParamDef {
        ParamDef {
            name: name.to_string(),
            original_name: name.to_string(),
            required,
            description: String::new(),
            location: ParamLocation::Query,
            schema: serde_json::json!({"type": "string"}),
        }
    }

    #[test]
    fn subcommand_generated() {
        let cmds = vec![make_cmd(vec![])];
        let app = build_command("mcpipe", &cmds);
        assert!(app.find_subcommand("list-pets").is_some());
    }

    #[test]
    fn extract_integer_arg() {
        let cmd = make_cmd(vec![int_param("limit", false)]);
        let app = build_command("mcpipe", &[cmd.clone()]);
        let matches = app.get_matches_from(["mcpipe", "list-pets", "--limit", "10"]);
        let (_, sub_matches) = matches.subcommand().unwrap();
        let args = extract_args(sub_matches, &cmd);
        assert_eq!(args["limit"], serde_json::json!(10i64));
    }

    #[test]
    fn extract_string_arg() {
        let cmd = make_cmd(vec![str_param("name", false)]);
        let app = build_command("mcpipe", &[cmd.clone()]);
        let matches = app.get_matches_from(["mcpipe", "list-pets", "--name", "rex"]);
        let (_, sub_matches) = matches.subcommand().unwrap();
        let args = extract_args(sub_matches, &cmd);
        assert_eq!(args["name"], serde_json::json!("rex"));
    }

    #[test]
    fn missing_optional_not_in_map() {
        let cmd = make_cmd(vec![str_param("name", false)]);
        let app = build_command("mcpipe", &[cmd.clone()]);
        let matches = app.get_matches_from(["mcpipe", "list-pets"]);
        let (_, sub_matches) = matches.subcommand().unwrap();
        let args = extract_args(sub_matches, &cmd);
        assert!(!args.contains_key("name"));
    }
}
```

Add to `src/main.rs`:

```rust
mod domain;
mod backend;
mod secret;
mod format;
mod cache;
mod cli;

fn main() {}
```

- [ ] **Step 2: Run tests**

```bash
cd /Users/joe/dev/mcpipe
cargo test cli
```

Expected: 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat: dynamic clap CLI builder from CommandDef list"
```

---

## Task 8: OpenAPI backend

**Files:**

- Modify: `src/backend/openapi.rs`
- Create: `tests/fixtures/petstore.json`
- Create: `tests/openapi_adapter.rs`

- [ ] **Step 1: Add petstore fixture**

Create `tests/fixtures/petstore.json`:

```json
{
  "openapi": "3.0.0",
  "info": { "title": "Petstore", "version": "1.0.0" },
  "paths": {
    "/pets": {
      "get": {
        "operationId": "listPets",
        "summary": "List all pets",
        "parameters": [
          {
            "name": "limit",
            "in": "query",
            "required": false,
            "schema": { "type": "integer" },
            "description": "How many items to return"
          }
        ]
      },
      "post": {
        "operationId": "createPet",
        "summary": "Create a pet",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                  "name": { "type": "string", "description": "Pet name" },
                  "tag": { "type": "string", "description": "Pet tag" }
                }
              }
            }
          }
        }
      }
    },
    "/pets/{petId}": {
      "get": {
        "operationId": "showPetById",
        "summary": "Info for a specific pet",
        "parameters": [
          {
            "name": "petId",
            "in": "path",
            "required": true,
            "schema": { "type": "string" },
            "description": "The id of the pet"
          }
        ]
      }
    }
  }
}
```

- [ ] **Step 2: Write failing adapter test**

Create `tests/openapi_adapter.rs`:

```rust
use mcpipe::backend::openapi::OpenApiBackend;
use mcpipe::backend::Backend;
use mcpipe::domain::ParamLocation;

#[tokio::test]
async fn discover_from_file() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/petstore.json");
    let backend = OpenApiBackend::from_file(path).unwrap();
    let cmds = backend.discover().await.unwrap();

    assert_eq!(cmds.len(), 3);

    let list = cmds.iter().find(|c| c.name == "list-pets").expect("list-pets");
    assert_eq!(list.source_name, "listPets");
    assert_eq!(list.params.len(), 1);
    assert_eq!(list.params[0].name, "limit");
    assert!(!list.params[0].required);
    assert!(matches!(list.params[0].location, ParamLocation::Query));

    let create = cmds.iter().find(|c| c.name == "create-pet").expect("create-pet");
    let name_param = create.params.iter().find(|p| p.name == "name").expect("name param");
    assert!(name_param.required);
    assert!(matches!(name_param.location, ParamLocation::Body));

    let show = cmds.iter().find(|c| c.name == "show-pet-by-id").expect("show-pet-by-id");
    let id_param = show.params.iter().find(|p| p.name == "pet-id").expect("petId param");
    assert!(id_param.required);
    assert!(matches!(id_param.location, ParamLocation::Path));
}
```

- [ ] **Step 3: Make `lib.rs` so tests can import**

Create `src/lib.rs`:

```rust
pub mod domain;
pub mod backend;
pub mod secret;
pub mod format;
pub mod cache;
pub mod cli;
```

Update `src/main.rs` to use the lib:

```rust
use mcpipe::domain;
use mcpipe::backend;
use mcpipe::secret;
use mcpipe::format;
use mcpipe::cache;
use mcpipe::cli;

fn main() {}
```

Update `Cargo.toml` to add lib target:

```toml
[lib]
name = "mcpipe"
path = "src/lib.rs"
```

- [ ] **Step 4: Run test to confirm it fails**

```bash
cd /Users/joe/dev/mcpipe
cargo test --test openapi_adapter 2>&1 | head -20
```

Expected: compile error — `OpenApiBackend` not yet implemented.

- [ ] **Step 5: Implement OpenApiBackend**

Replace `src/backend/openapi.rs` with:

```rust
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;

use crate::domain::{BackendError, CommandDef, ArgMap, ParamDef, ParamLocation};
use super::Backend;

pub struct OpenApiBackend {
    spec: serde_json::Value,
    base_url: String,
    auth_headers: Vec<(String, String)>,
}

impl OpenApiBackend {
    pub fn from_file(path: &str) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("reading spec file {path}"))?;
        let spec: serde_json::Value = serde_json::from_str(&data)
            .context("parsing OpenAPI spec JSON")?;
        let base_url = extract_base_url(&spec);
        Ok(Self { spec, base_url, auth_headers: vec![] })
    }

    pub fn from_json(spec: serde_json::Value, base_url: String, auth_headers: Vec<(String, String)>) -> Self {
        Self { spec, base_url, auth_headers }
    }

    fn build_commands(&self) -> Result<Vec<CommandDef>, BackendError> {
        let spec = resolve_refs(&self.spec);
        let paths = spec.get("paths")
            .and_then(|p| p.as_object())
            .ok_or_else(|| BackendError::Schema("no paths in spec".to_string()))?;

        let mut cmds = vec![];

        for (path, path_item) in paths {
            let path_item = path_item.as_object()
                .ok_or_else(|| BackendError::Schema(format!("invalid path item for {path}")))?;

            for method in &["get", "post", "put", "patch", "delete"] {
                let Some(op) = path_item.get(*method) else { continue };

                let operation_id = op.get("operationId")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&format!("{}-{}", method, path.trim_matches('/')));

                let name = to_kebab(operation_id);
                let description = op.get("summary")
                    .or_else(|| op.get("description"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let mut params = vec![];

                // Path + query + header params
                if let Some(parameters) = op.get("parameters").and_then(|p| p.as_array()) {
                    for p in parameters {
                        let pname = p.get("name").and_then(|v| v.as_str()).unwrap_or("param");
                        let location = match p.get("in").and_then(|v| v.as_str()) {
                            Some("query")  => ParamLocation::Query,
                            Some("path")   => ParamLocation::Path,
                            Some("header") => ParamLocation::Header,
                            _              => ParamLocation::Body,
                        };
                        let required = p.get("required")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(matches!(location, ParamLocation::Path));
                        let schema = p.get("schema").cloned().unwrap_or(serde_json::json!({"type":"string"}));
                        let desc = p.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();

                        params.push(ParamDef {
                            name: to_kebab(pname),
                            original_name: pname.to_string(),
                            required,
                            description: desc,
                            location,
                            schema,
                        });
                    }
                }

                // Request body params (application/json schema properties)
                if let Some(body) = op.get("requestBody") {
                    let required_body = body.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
                    let schema = body
                        .pointer("/content/application~1json/schema")
                        .or_else(|| body.pointer("/content/application\\/json/schema"))
                        .cloned();

                    if let Some(schema) = schema {
                        let required_fields: Vec<&str> = schema
                            .get("required")
                            .and_then(|v| v.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                            .unwrap_or_default();

                        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
                            for (prop_name, prop_schema) in props {
                                let required = required_fields.contains(&prop_name.as_str());
                                let desc = prop_schema.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                params.push(ParamDef {
                                    name: to_kebab(prop_name),
                                    original_name: prop_name.clone(),
                                    required,
                                    description: desc,
                                    location: ParamLocation::Body,
                                    schema: prop_schema.clone(),
                                });
                            }
                        }
                    }
                }

                cmds.push(CommandDef {
                    name,
                    description,
                    source_name: operation_id.to_string(),
                    params,
                });
            }
        }

        Ok(cmds)
    }
}

#[async_trait]
impl Backend for OpenApiBackend {
    async fn discover(&self) -> Result<Vec<CommandDef>, BackendError> {
        self.build_commands()
    }

    async fn execute(&self, cmd: &CommandDef, args: ArgMap) -> Result<serde_json::Value, BackendError> {
        // Build URL by substituting path params
        let paths = self.spec.get("paths")
            .and_then(|p| p.as_object())
            .ok_or_else(|| BackendError::Schema("no paths".to_string()))?;

        // Find the matching path+method for this operation
        let (path_template, method) = find_operation(&self.spec, &cmd.source_name)
            .ok_or_else(|| BackendError::NotFound(cmd.source_name.clone()))?;

        let mut url_path = path_template.clone();
        let mut query_params = vec![];
        let mut body_map: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();

        for param in &cmd.params {
            let val = match args.get(&param.original_name) {
                Some(v) => v.clone(),
                None => continue,
            };
            match param.location {
                ParamLocation::Path => {
                    url_path = url_path.replace(
                        &format!("{{{}}}", param.original_name),
                        val.as_str().unwrap_or(&val.to_string()),
                    );
                }
                ParamLocation::Query => {
                    query_params.push((param.original_name.clone(), val.to_string().trim_matches('"').to_string()));
                }
                ParamLocation::Body => {
                    body_map.insert(param.original_name.clone(), val);
                }
                ParamLocation::Header | ParamLocation::ToolInput => {}
            }
        }

        let url = format!("{}{}", self.base_url.trim_end_matches('/'), url_path);

        let client = reqwest::Client::new();
        let mut req = match method.as_str() {
            "get"    => client.get(&url),
            "post"   => client.post(&url),
            "put"    => client.put(&url),
            "patch"  => client.patch(&url),
            "delete" => client.delete(&url),
            _        => client.get(&url),
        };

        for (k, v) in &self.auth_headers {
            req = req.header(k, v);
        }

        if !query_params.is_empty() {
            req = req.query(&query_params);
        }

        if !body_map.is_empty() {
            req = req.json(&body_map);
        }

        let resp = req.send().await
            .map_err(|e| BackendError::Transport(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(BackendError::Execution(format!("HTTP {}: {}", resp.status(), resp.text().await.unwrap_or_default())));
        }

        let val: serde_json::Value = resp.json().await
            .map_err(|e| BackendError::Execution(e.to_string()))?;

        Ok(val)
    }
}

fn find_operation(spec: &serde_json::Value, operation_id: &str) -> Option<(String, String)> {
    let paths = spec.get("paths")?.as_object()?;
    for (path, path_item) in paths {
        let pi = path_item.as_object()?;
        for method in &["get", "post", "put", "patch", "delete"] {
            if let Some(op) = pi.get(*method) {
                let oid = op.get("operationId").and_then(|v| v.as_str()).unwrap_or("");
                if oid == operation_id {
                    return Some((path.clone(), method.to_string()));
                }
            }
        }
    }
    None
}

fn extract_base_url(spec: &serde_json::Value) -> String {
    spec.pointer("/servers/0/url")
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost")
        .to_string()
}

/// Resolve $ref pointers within a spec (local #/ refs only).
pub fn resolve_refs(spec: &serde_json::Value) -> serde_json::Value {
    resolve_node(spec, spec)
}

fn resolve_node(node: &serde_json::Value, root: &serde_json::Value) -> serde_json::Value {
    match node {
        serde_json::Value::Object(map) => {
            if let Some(ref_val) = map.get("$ref").and_then(|v| v.as_str()) {
                if let Some(resolved) = resolve_ref(ref_val, root) {
                    return resolve_node(&resolved, root);
                }
            }
            serde_json::Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), resolve_node(v, root)))
                    .collect(),
            )
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(|v| resolve_node(v, root)).collect())
        }
        other => other.clone(),
    }
}

fn resolve_ref(ref_str: &str, root: &serde_json::Value) -> Option<serde_json::Value> {
    let path = ref_str.strip_prefix("#/")?;
    let mut cur = root;
    for part in path.split('/') {
        cur = cur.get(part)?;
    }
    Some(cur.clone())
}

pub fn to_kebab(s: &str) -> String {
    // camelCase → kebab-case, then replace underscores
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('-');
        }
        out.push(c.to_lowercase().next().unwrap());
    }
    out.replace('_', "-")
}
```

- [ ] **Step 6: Run adapter test**

```bash
cd /Users/joe/dev/mcpipe
cargo test --test openapi_adapter
```

Expected: `discover_from_file ... ok`

- [ ] **Step 7: Commit**

```bash
git add src/backend/openapi.rs src/lib.rs src/main.rs Cargo.toml tests/fixtures/petstore.json tests/openapi_adapter.rs
git commit -m "feat: OpenApiBackend — spec load, \$ref resolve, CommandDef generation, execute"
```

---

## Task 9: GraphQL backend

**Files:**

- Modify: `src/backend/graphql.rs`
- Create: `tests/fixtures/introspection.json`
- Create: `tests/graphql_adapter.rs`

- [ ] **Step 1: Add introspection fixture**

Create `tests/fixtures/introspection.json`:

```json
{
  "data": {
    "__schema": {
      "queryType": { "name": "Query" },
      "mutationType": { "name": "Mutation" },
      "types": [
        {
          "name": "Query",
          "fields": [
            {
              "name": "pets",
              "description": "List all pets",
              "args": [
                {
                  "name": "limit",
                  "description": "Max results",
                  "type": { "kind": "SCALAR", "name": "Int", "ofType": null },
                  "defaultValue": null
                }
              ],
              "type": {
                "kind": "LIST",
                "name": null,
                "ofType": { "kind": "OBJECT", "name": "Pet" }
              }
            }
          ]
        },
        {
          "name": "Mutation",
          "fields": [
            {
              "name": "createPet",
              "description": "Create a pet",
              "args": [
                {
                  "name": "name",
                  "description": "Pet name",
                  "type": {
                    "kind": "NON_NULL",
                    "name": null,
                    "ofType": { "kind": "SCALAR", "name": "String" }
                  },
                  "defaultValue": null
                }
              ],
              "type": { "kind": "OBJECT", "name": "Pet" }
            }
          ]
        },
        {
          "name": "Pet",
          "fields": [
            {
              "name": "id",
              "description": "",
              "args": [],
              "type": { "kind": "SCALAR", "name": "String" }
            },
            {
              "name": "name",
              "description": "",
              "args": [],
              "type": { "kind": "SCALAR", "name": "String" }
            }
          ]
        }
      ]
    }
  }
}
```

- [ ] **Step 2: Write failing adapter test**

Create `tests/graphql_adapter.rs`:

```rust
use mcpipe::backend::graphql::GraphQlBackend;
use mcpipe::backend::Backend;
use mcpipe::domain::ParamLocation;

#[tokio::test]
async fn discover_from_introspection_fixture() {
    let fixture = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/introspection.json")
    ).unwrap();
    let introspection: serde_json::Value = serde_json::from_str(&fixture).unwrap();

    let backend = GraphQlBackend::from_introspection(
        "http://localhost:4000/graphql".to_string(),
        introspection,
        vec![],
    );

    let cmds = backend.discover().await.unwrap();
    assert_eq!(cmds.len(), 2, "expected pets + createPet");

    let pets = cmds.iter().find(|c| c.name == "pets").expect("pets query");
    assert_eq!(pets.params.len(), 1);
    assert_eq!(pets.params[0].name, "limit");
    assert!(!pets.params[0].required);

    let create = cmds.iter().find(|c| c.name == "create-pet").expect("createPet mutation");
    let name_p = create.params.iter().find(|p| p.name == "name").expect("name param");
    assert!(name_p.required);
}
```

- [ ] **Step 3: Run test to confirm it fails**

```bash
cd /Users/joe/dev/mcpipe
cargo test --test graphql_adapter 2>&1 | head -10
```

Expected: compile error — `GraphQlBackend` not implemented.

- [ ] **Step 4: Implement GraphQlBackend**

Replace `src/backend/graphql.rs`:

```rust
use async_trait::async_trait;
use anyhow::Context;

use crate::domain::{ArgMap, BackendError, CommandDef, ParamDef, ParamLocation};
use super::Backend;
use crate::backend::openapi::to_kebab;

pub struct GraphQlBackend {
    endpoint: String,
    introspection: Option<serde_json::Value>,
    auth_headers: Vec<(String, String)>,
    fields_override: Option<String>,
}

impl GraphQlBackend {
    pub fn new(endpoint: String, auth_headers: Vec<(String, String)>) -> Self {
        Self { endpoint, introspection: None, auth_headers, fields_override: None }
    }

    pub fn from_introspection(
        endpoint: String,
        introspection: serde_json::Value,
        auth_headers: Vec<(String, String)>,
    ) -> Self {
        Self { endpoint, introspection: Some(introspection), auth_headers, fields_override: None }
    }

    pub fn with_fields_override(mut self, fields: String) -> Self {
        self.fields_override = Some(fields);
        self
    }

    async fn fetch_introspection(&self) -> Result<serde_json::Value, BackendError> {
        const INTROSPECTION_QUERY: &str = r#"
        {
          __schema {
            queryType { name }
            mutationType { name }
            types {
              name
              fields(includeDeprecated: false) {
                name
                description
                args {
                  name
                  description
                  type { kind name ofType { kind name ofType { kind name } } }
                  defaultValue
                }
                type { kind name ofType { kind name ofType { kind name } } }
              }
            }
          }
        }"#;

        let client = reqwest::Client::new();
        let mut req = client.post(&self.endpoint)
            .json(&serde_json::json!({"query": INTROSPECTION_QUERY}));
        for (k, v) in &self.auth_headers {
            req = req.header(k, v);
        }
        let resp = req.send().await
            .map_err(|e| BackendError::Transport(e.to_string()))?;
        let val: serde_json::Value = resp.json().await
            .map_err(|e| BackendError::Schema(e.to_string()))?;
        Ok(val)
    }

    fn build_commands(&self, introspection: &serde_json::Value) -> Result<Vec<CommandDef>, BackendError> {
        let schema = introspection.pointer("/data/__schema")
            .ok_or_else(|| BackendError::Schema("no __schema in introspection".to_string()))?;

        let query_type = schema.pointer("/queryType/name").and_then(|v| v.as_str()).unwrap_or("Query");
        let mutation_type = schema.pointer("/mutationType/name").and_then(|v| v.as_str());

        let types = schema.get("types")
            .and_then(|t| t.as_array())
            .ok_or_else(|| BackendError::Schema("no types array".to_string()))?;

        let mut cmds = vec![];

        for type_def in types {
            let type_name = type_def.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let is_query = type_name == query_type;
            let is_mutation = mutation_type.map_or(false, |mt| type_name == mt);

            if !is_query && !is_mutation {
                continue;
            }

            let fields = match type_def.get("fields").and_then(|f| f.as_array()) {
                Some(f) => f,
                None => continue,
            };

            for field in fields {
                let field_name = field.get("name").and_then(|v| v.as_str()).unwrap_or("op");
                let description = field.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();

                let args = field.get("args").and_then(|a| a.as_array()).map(|a| a.as_slice()).unwrap_or(&[]);
                let mut params = vec![];

                for arg in args {
                    let aname = arg.get("name").and_then(|v| v.as_str()).unwrap_or("arg");
                    let adesc = arg.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let required = is_non_null(arg.get("type").unwrap_or(&serde_json::Value::Null));
                    let schema = graphql_type_to_json_schema(arg.get("type").unwrap_or(&serde_json::Value::Null));

                    params.push(ParamDef {
                        name: to_kebab(aname),
                        original_name: aname.to_string(),
                        required,
                        description: adesc,
                        location: ParamLocation::ToolInput,
                        schema,
                    });
                }

                cmds.push(CommandDef {
                    name: to_kebab(field_name),
                    description,
                    source_name: field_name.to_string(),
                    params,
                });
            }
        }

        Ok(cmds)
    }
}

#[async_trait]
impl Backend for GraphQlBackend {
    async fn discover(&self) -> Result<Vec<CommandDef>, BackendError> {
        let intro = match &self.introspection {
            Some(i) => i.clone(),
            None => self.fetch_introspection().await?,
        };
        self.build_commands(&intro)
    }

    async fn execute(&self, cmd: &CommandDef, args: ArgMap) -> Result<serde_json::Value, BackendError> {
        // Build a simple query/mutation string
        let arg_str: String = args.iter()
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect::<Vec<_>>()
            .join(", ");

        let call = if arg_str.is_empty() {
            cmd.source_name.clone()
        } else {
            format!("{}({})", cmd.source_name, arg_str)
        };

        let fields = self.fields_override.clone().unwrap_or_else(|| "id".to_string());
        let query = format!("{{ {} {{ {} }} }}", call, fields);

        let client = reqwest::Client::new();
        let mut req = client.post(&self.endpoint)
            .json(&serde_json::json!({"query": query}));
        for (k, v) in &self.auth_headers {
            req = req.header(k, v);
        }

        let resp = req.send().await
            .map_err(|e| BackendError::Transport(e.to_string()))?;
        let val: serde_json::Value = resp.json().await
            .map_err(|e| BackendError::Execution(e.to_string()))?;

        if let Some(errors) = val.get("errors") {
            return Err(BackendError::Execution(errors.to_string()));
        }

        Ok(val.pointer(&format!("/data/{}", cmd.source_name)).cloned()
            .unwrap_or(val))
    }
}

fn is_non_null(type_val: &serde_json::Value) -> bool {
    type_val.get("kind").and_then(|v| v.as_str()) == Some("NON_NULL")
}

fn graphql_type_to_json_schema(type_val: &serde_json::Value) -> serde_json::Value {
    let name = type_val.get("name").and_then(|v| v.as_str())
        .or_else(|| type_val.pointer("/ofType/name").and_then(|v| v.as_str()))
        .unwrap_or("String");

    match name {
        "Int"     => serde_json::json!({"type": "integer"}),
        "Float"   => serde_json::json!({"type": "number"}),
        "Boolean" => serde_json::json!({"type": "boolean"}),
        _         => serde_json::json!({"type": "string"}),
    }
}
```

- [ ] **Step 5: Run adapter test**

```bash
cd /Users/joe/dev/mcpipe
cargo test --test graphql_adapter
```

Expected: `discover_from_introspection_fixture ... ok`

- [ ] **Step 6: Commit**

```bash
git add src/backend/graphql.rs tests/fixtures/introspection.json tests/graphql_adapter.rs
git commit -m "feat: GraphQlBackend — introspection discovery, CommandDef generation, execute"
```

---

## Task 10: MCP backend (stdio)

**Files:**

- Modify: `src/backend/mcp.rs`
- Create: `tests/fixtures/mcp_echo.py`
- Create: `tests/mcp_adapter.rs`

- [ ] **Step 1: Write MCP echo server fixture**

Create `tests/fixtures/mcp_echo.py`:

```python
#!/usr/bin/env python3
"""Minimal MCP stdio server for testing.
Responds to tools/list and tools/call over stdin/stdout (JSON-RPC 2.0).
"""
import json
import sys

TOOLS = [
    {
        "name": "echo",
        "description": "Echo the input back",
        "inputSchema": {
            "type": "object",
            "required": ["message"],
            "properties": {
                "message": {"type": "string", "description": "Text to echo"}
            }
        }
    }
]

def handle(req):
    method = req.get("method", "")
    rid = req.get("id")

    if method == "initialize":
        return {"jsonrpc": "2.0", "id": rid, "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "echo-server", "version": "0.1.0"}
        }}
    if method == "tools/list":
        return {"jsonrpc": "2.0", "id": rid, "result": {"tools": TOOLS}}
    if method == "tools/call":
        tool = req.get("params", {}).get("name", "")
        args = req.get("params", {}).get("arguments", {})
        if tool == "echo":
            return {"jsonrpc": "2.0", "id": rid, "result": {
                "content": [{"type": "text", "text": args.get("message", "")}]
            }}
        return {"jsonrpc": "2.0", "id": rid, "error": {"code": -32601, "message": "tool not found"}}
    if method == "notifications/initialized":
        return None  # no response for notifications
    return {"jsonrpc": "2.0", "id": rid, "error": {"code": -32601, "message": "method not found"}}

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except json.JSONDecodeError:
        continue
    resp = handle(req)
    if resp is not None:
        print(json.dumps(resp), flush=True)
```

- [ ] **Step 2: Write failing adapter test**

Create `tests/mcp_adapter.rs`:

```rust
use mcpipe::backend::mcp::McpBackend;
use mcpipe::backend::Backend;
use mcpipe::domain::ParamLocation;

#[tokio::test]
async fn mcp_stdio_discover() {
    let python = which_python();
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mcp_echo.py");
    let cmd = format!("{python} {script}");

    let backend = McpBackend::from_stdio(cmd);
    let cmds = backend.discover().await.unwrap();

    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].name, "echo");
    assert_eq!(cmds[0].params.len(), 1);
    assert_eq!(cmds[0].params[0].name, "message");
    assert!(cmds[0].params[0].required);
    assert!(matches!(cmds[0].params[0].location, ParamLocation::ToolInput));
}

#[tokio::test]
async fn mcp_stdio_execute() {
    let python = which_python();
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mcp_echo.py");
    let cmd = format!("{python} {script}");

    let backend = McpBackend::from_stdio(cmd);
    let cmds = backend.discover().await.unwrap();
    let echo_cmd = cmds.iter().find(|c| c.name == "echo").unwrap();

    let mut args = std::collections::HashMap::new();
    args.insert("message".to_string(), serde_json::json!("hello mcpipe"));

    let result = backend.execute(echo_cmd, args).await.unwrap();
    // result should be the text content
    let text = result.pointer("/0/text")
        .or_else(|| result.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| result.as_str().unwrap_or(""));
    assert_eq!(text, "hello mcpipe");
}

fn which_python() -> String {
    for candidate in &["python3", "python"] {
        if std::process::Command::new(candidate)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return candidate.to_string();
        }
    }
    panic!("python3 not found — required for MCP adapter tests");
}
```

- [ ] **Step 3: Implement McpBackend (stdio)**

Replace `src/backend/mcp.rs`:

```rust
use async_trait::async_trait;
use anyhow::Context;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use std::sync::Arc;

use crate::domain::{ArgMap, BackendError, CommandDef, ParamDef, ParamLocation};
use crate::backend::openapi::to_kebab;
use super::Backend;

pub struct McpBackend {
    inner: McpTransport,
}

enum McpTransport {
    Stdio { command: String },
    Http  { url: String, auth_headers: Vec<(String, String)> },
}

impl McpBackend {
    pub fn from_stdio(command: String) -> Self {
        Self { inner: McpTransport::Stdio { command } }
    }

    pub fn from_http(url: String, auth_headers: Vec<(String, String)>) -> Self {
        Self { inner: McpTransport::Http { url, auth_headers } }
    }

    async fn run_stdio_session<F, Fut>(&self, command: &str, f: F) -> Result<serde_json::Value, BackendError>
    where
        F: FnOnce(StdioSession) -> Fut,
        Fut: std::future::Future<Output = Result<serde_json::Value, BackendError>>,
    {
        let mut parts = command.split_whitespace();
        let prog = parts.next().ok_or_else(|| BackendError::Transport("empty command".to_string()))?;
        let args: Vec<&str> = parts.collect();

        let mut child = Command::new(prog)
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| BackendError::Transport(format!("spawn {command}: {e}")))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| BackendError::Transport("no stdin".to_string()))?;
        let stdout = child.stdout.take()
            .ok_or_else(|| BackendError::Transport("no stdout".to_string()))?;

        let session = StdioSession::new(stdin, stdout);

        // Send initialize
        session.send_initialize().await?;

        let result = f(session).await;

        // Kill child process
        let _ = child.kill().await;

        result
    }
}

struct StdioSession {
    stdin: Mutex<tokio::process::ChildStdin>,
    reader: Mutex<BufReader<tokio::process::ChildStdout>>,
    next_id: Mutex<u64>,
}

impl StdioSession {
    fn new(stdin: tokio::process::ChildStdin, stdout: tokio::process::ChildStdout) -> Self {
        Self {
            stdin: Mutex::new(stdin),
            reader: Mutex::new(BufReader::new(stdout)),
            next_id: Mutex::new(1),
        }
    }

    async fn next_id(&self) -> u64 {
        let mut id = self.next_id.lock().await;
        let cur = *id;
        *id += 1;
        cur
    }

    async fn send_request(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, BackendError> {
        let id = self.next_id().await;
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let line = format!("{}\n", serde_json::to_string(&msg).unwrap());

        self.stdin.lock().await
            .write_all(line.as_bytes()).await
            .map_err(|e| BackendError::Transport(format!("write: {e}")))?;

        // Read response lines until we get one with matching id
        let mut reader = self.reader.lock().await;
        loop {
            let mut buf = String::new();
            reader.read_line(&mut buf).await
                .map_err(|e| BackendError::Transport(format!("read: {e}")))?;
            if buf.is_empty() {
                return Err(BackendError::Transport("EOF from MCP server".to_string()));
            }
            let val: serde_json::Value = match serde_json::from_str(buf.trim()) {
                Ok(v) => v,
                Err(_) => continue,
            };
            // Check it's a response to our request
            if val.get("id").and_then(|v| v.as_u64()) == Some(id) {
                if let Some(err) = val.get("error") {
                    return Err(BackendError::Execution(err.to_string()));
                }
                return Ok(val.get("result").cloned().unwrap_or(serde_json::Value::Null));
            }
            // Otherwise it's a notification or different id — skip
        }
    }

    async fn send_notification(&self, method: &str, params: serde_json::Value) -> Result<(), BackendError> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let line = format!("{}\n", serde_json::to_string(&msg).unwrap());
        self.stdin.lock().await
            .write_all(line.as_bytes()).await
            .map_err(|e| BackendError::Transport(format!("write notification: {e}")))?;
        Ok(())
    }

    async fn send_initialize(&self) -> Result<(), BackendError> {
        self.send_request("initialize", serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "mcpipe", "version": "0.1.0" }
        })).await?;
        self.send_notification("notifications/initialized", serde_json::json!({})).await?;
        Ok(())
    }
}

fn tools_to_commands(tools: &[serde_json::Value]) -> Vec<CommandDef> {
    tools.iter().map(|tool| {
        let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
        let description = tool.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let schema = tool.get("inputSchema").cloned().unwrap_or(serde_json::json!({}));

        let mut params = vec![];
        let required_fields: Vec<&str> = schema.get("required")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            for (pname, pschema) in props {
                let required = required_fields.contains(&pname.as_str());
                let desc = pschema.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                params.push(ParamDef {
                    name: to_kebab(pname),
                    original_name: pname.clone(),
                    required,
                    description: desc,
                    location: ParamLocation::ToolInput,
                    schema: pschema.clone(),
                });
            }
        }

        CommandDef {
            name: to_kebab(name),
            description,
            source_name: name.to_string(),
            params,
        }
    }).collect()
}

#[async_trait]
impl Backend for McpBackend {
    async fn discover(&self) -> Result<Vec<CommandDef>, BackendError> {
        match &self.inner {
            McpTransport::Stdio { command } => {
                let command = command.clone();
                self.run_stdio_session(&command, |session| async move {
                    let result = session.send_request("tools/list", serde_json::json!({})).await?;
                    let tools = result.get("tools")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    Ok(serde_json::json!(tools_to_commands(&tools)))
                }).await
                .map(|v| serde_json::from_value(v).unwrap_or_default())
            }
            McpTransport::Http { url, auth_headers } => {
                Err(BackendError::Transport("HTTP MCP not yet implemented".to_string()))
            }
        }
    }

    async fn execute(&self, cmd: &CommandDef, args: ArgMap) -> Result<serde_json::Value, BackendError> {
        match &self.inner {
            McpTransport::Stdio { command } => {
                let command = command.clone();
                let tool_name = cmd.source_name.clone();
                self.run_stdio_session(&command, |session| async move {
                    let result = session.send_request("tools/call", serde_json::json!({
                        "name": tool_name,
                        "arguments": args,
                    })).await?;
                    // MCP returns { content: [{type, text}] }
                    Ok(result.get("content").cloned().unwrap_or(result))
                }).await
            }
            McpTransport::Http { .. } => {
                Err(BackendError::Transport("HTTP MCP not yet implemented".to_string()))
            }
        }
    }
}
```

- [ ] **Step 4: Run adapter tests**

```bash
cd /Users/joe/dev/mcpipe
cargo test --test mcp_adapter
```

Expected: `mcp_stdio_discover ... ok`, `mcp_stdio_execute ... ok`

- [ ] **Step 5: Commit**

```bash
git add src/backend/mcp.rs tests/fixtures/mcp_echo.py tests/mcp_adapter.rs
git commit -m "feat: McpBackend stdio — JSON-RPC session, discover, execute"
```

---

## Task 11: Wire main.rs

**Files:**

- Modify: `src/main.rs`

- [ ] **Step 1: Write the full main.rs**

Replace `src/main.rs` with:

```rust
use anyhow::{bail, Context, Result};
use clap::{Arg, ArgAction, ArgMatches, Command};
use std::time::Duration;

use mcpipe::backend::mcp::McpBackend;
use mcpipe::backend::openapi::OpenApiBackend;
use mcpipe::backend::graphql::GraphQlBackend;
use mcpipe::backend::Backend;
use mcpipe::cache::Cache;
use mcpipe::cli::{build_command, extract_args};
use mcpipe::format::{format_value, FormatOptions};
use mcpipe::secret::resolve_secret;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let app = build_global_parser();
    let matches = app.get_matches();

    // Output options
    let pretty = matches.get_flag("pretty");
    let raw = matches.get_flag("raw");
    let jq = matches.get_one::<String>("jq").cloned();
    let head = matches.get_one::<usize>("head").copied();
    let list_only = matches.get_flag("list");
    let search = matches.get_one::<String>("search").cloned();
    let refresh = matches.get_flag("refresh");
    let cache_ttl = *matches.get_one::<u64>("cache-ttl").unwrap_or(&3600);

    // Auth headers
    let auth_headers: Vec<(String, String)> = matches
        .get_many::<String>("auth-header")
        .unwrap_or_default()
        .filter_map(|h| {
            let (k, v) = h.split_once(':')?;
            Some((k.trim().to_string(), resolve_secret(v.trim()).ok()?))
        })
        .collect();

    // Build backend
    let backend: Box<dyn Backend> = if let Some(cmd) = matches.get_one::<String>("mcp-stdio") {
        Box::new(McpBackend::from_stdio(cmd.clone()))
    } else if let Some(url) = matches.get_one::<String>("mcp") {
        Box::new(McpBackend::from_http(url.clone(), auth_headers.clone()))
    } else if let Some(url) = matches.get_one::<String>("graphql") {
        let mut b = GraphQlBackend::new(url.clone(), auth_headers.clone());
        if let Some(fields) = matches.get_one::<String>("fields") {
            b = b.with_fields_override(fields.clone());
        }
        Box::new(b)
    } else if let Some(spec) = matches.get_one::<String>("spec") {
        if spec.starts_with("http://") || spec.starts_with("https://") {
            let spec_json = fetch_spec(spec, &auth_headers).await?;
            let base_url = spec_json.pointer("/servers/0/url")
                .and_then(|v| v.as_str())
                .unwrap_or("http://localhost")
                .to_string();
            Box::new(OpenApiBackend::from_json(spec_json, base_url, auth_headers.clone()))
        } else {
            Box::new(OpenApiBackend::from_file(spec).context("loading OpenAPI spec")?)
        }
    } else {
        bail!("specify one of --mcp-stdio, --mcp, --spec, or --graphql");
    };

    // Cache key (for non-stdio sources)
    let cache_source = matches.get_one::<String>("mcp")
        .or_else(|| matches.get_one::<String>("spec"))
        .or_else(|| matches.get_one::<String>("graphql"))
        .cloned();

    let cache = Cache::new(Cache::default_dir(), Duration::from_secs(cache_ttl));

    // Discover commands (with cache)
    let cmds = if let Some(ref source) = cache_source {
        if !refresh {
            if let Some(cached) = cache.load(source) {
                cached
            } else {
                let discovered = backend.discover().await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                let _ = cache.save(source, &discovered);
                discovered
            }
        } else {
            let discovered = backend.discover().await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let _ = cache.save(source, &discovered);
            discovered
        }
    } else {
        backend.discover().await.map_err(|e| anyhow::anyhow!("{e}"))?
    };

    // --list / --search
    if list_only || search.is_some() {
        let pattern = search.as_deref().unwrap_or("").to_lowercase();
        for cmd in &cmds {
            if pattern.is_empty() || cmd.name.contains(&pattern) || cmd.description.to_lowercase().contains(&pattern) {
                println!("{:30}  {}", cmd.name, cmd.description);
            }
        }
        return Ok(());
    }

    // Build dynamic CLI and parse remaining args
    let dynamic = build_command("mcpipe", &cmds);

    // Re-parse with dynamic subcommands
    let all_args: Vec<String> = std::env::args().collect();
    // Find position after global flags to pass to dynamic parser
    // Use get_many raw_args workaround: re-run clap on remaining args
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let global_flags = [
        "--mcp-stdio", "--mcp", "--spec", "--graphql",
        "--pretty", "--raw", "--refresh", "--list",
        "--auth-header", "--cache-ttl", "--jq", "--head", "--search", "--fields",
    ];

    // Strip global flags to leave only subcommand args
    let mut tool_args = vec!["mcpipe".to_string()];
    let mut skip_next = false;
    for arg in &raw {
        if skip_next { skip_next = false; continue; }
        let is_global_value_flag = ["--mcp-stdio","--mcp","--spec","--graphql","--auth-header","--cache-ttl","--jq","--head","--search","--fields"]
            .iter().any(|f| arg.as_str() == *f);
        let is_global_bool_flag = ["--pretty","--raw","--refresh","--list"]
            .iter().any(|f| arg.as_str() == *f);
        if is_global_value_flag { skip_next = true; continue; }
        if is_global_bool_flag { continue; }
        tool_args.push(arg.clone());
    }

    let dynamic_matches = dynamic.try_get_matches_from(&tool_args)
        .map_err(|e| { e.print().ok(); anyhow::anyhow!("") })?;

    let (sub_name, sub_matches) = dynamic_matches.subcommand()
        .ok_or_else(|| anyhow::anyhow!("no subcommand provided — use --list to see available commands"))?;

    let cmd_def = cmds.iter().find(|c| c.name == sub_name)
        .ok_or_else(|| anyhow::anyhow!("unknown command: {sub_name}"))?;

    let args = extract_args(sub_matches, cmd_def);
    let result = backend.execute(cmd_def, args).await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let opts = FormatOptions { pretty, raw, jq, head };
    let output = format_value(&result, &opts)?;
    println!("{output}");

    Ok(())
}

fn build_global_parser() -> Command {
    Command::new("mcpipe")
        .about("Turn any MCP server, OpenAPI spec, or GraphQL endpoint into a shell CLI")
        .arg(Arg::new("mcp-stdio").long("mcp-stdio").value_name("CMD").help("MCP server command (stdio)"))
        .arg(Arg::new("mcp").long("mcp").value_name("URL").help("MCP server URL (HTTP/SSE)"))
        .arg(Arg::new("spec").long("spec").value_name("URL_OR_FILE").help("OpenAPI spec URL or file path"))
        .arg(Arg::new("graphql").long("graphql").value_name("URL").help("GraphQL endpoint URL"))
        .arg(Arg::new("auth-header").long("auth-header").value_name("NAME:VALUE").action(ArgAction::Append).help("Auth header (repeatable). Value supports env:VAR and file:/path"))
        .arg(Arg::new("pretty").long("pretty").action(ArgAction::SetTrue).help("Pretty-print JSON output"))
        .arg(Arg::new("raw").long("raw").action(ArgAction::SetTrue).help("Print raw string values"))
        .arg(Arg::new("refresh").long("refresh").action(ArgAction::SetTrue).help("Bypass cache, re-fetch"))
        .arg(Arg::new("list").long("list").action(ArgAction::SetTrue).help("List available subcommands"))
        .arg(Arg::new("search").long("search").value_name("PATTERN").help("Search commands by name/description"))
        .arg(Arg::new("cache-ttl").long("cache-ttl").value_name("SECS").value_parser(clap::value_parser!(u64)).help("Cache TTL in seconds (default: 3600)"))
        .arg(Arg::new("jq").long("jq").value_name("EXPR").help("Filter output through jq"))
        .arg(Arg::new("head").long("head").value_name("N").value_parser(clap::value_parser!(usize)).help("Limit output to first N array elements"))
        .arg(Arg::new("fields").long("fields").value_name("FIELDS").help("Override GraphQL selection set fields"))
        .allow_external_subcommands(true)
}

async fn fetch_spec(url: &str, auth_headers: &[(String, String)]) -> Result<serde_json::Value> {
    let client = reqwest::Client::new();
    let mut req = client.get(url);
    for (k, v) in auth_headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let resp = req.send().await.context("fetching spec")?;
    if !resp.status().is_success() {
        bail!("HTTP {}: {}", resp.status(), resp.text().await.unwrap_or_default());
    }
    resp.json().await.context("parsing spec JSON")
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cd /Users/joe/dev/mcpipe
cargo build 2>&1
```

Fix any errors. Expected: builds cleanly.

- [ ] **Step 3: Smoke test with the petstore fixture**

```bash
cd /Users/joe/dev/mcpipe
./target/debug/mcpipe --spec tests/fixtures/petstore.json --list
```

Expected output (order may vary):

```
list-pets                       List all pets
create-pet                      Create a pet
show-pet-by-id                  Info for a specific pet
```

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire main.rs — global flags, backend dispatch, discover/execute/format pipeline"
```

---

## Task 12: Run all tests and final check

- [ ] **Step 1: Run full test suite**

```bash
cd /Users/joe/dev/mcpipe
cargo test
```

Expected: all tests pass (unit + adapter tests).

- [ ] **Step 2: Run clippy**

```bash
cd /Users/joe/dev/mcpipe
cargo clippy -- -D warnings
```

Fix any warnings before continuing.

- [ ] **Step 3: Check binary size and confirm single binary**

```bash
cd /Users/joe/dev/mcpipe
cargo build --release
ls -lh target/release/mcpipe
file target/release/mcpipe
```

Expected: single binary, no dynamic MCP/Python runtime required.

- [ ] **Step 4: Final smoke test with MCP stdio**

```bash
cd /Users/joe/dev/mcpipe
./target/release/mcpipe --mcp-stdio "python3 tests/fixtures/mcp_echo.py" --list
```

Expected:

```
echo                            Echo the input back
```

```bash
./target/release/mcpipe --mcp-stdio "python3 tests/fixtures/mcp_echo.py" echo --message "hello world"
```

Expected: `[{"type":"text","text":"hello world"}]`

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: all tests passing, clippy clean"
```

---

## Self-Review

**Spec coverage check:**

| Spec requirement                      | Task                                        |
| ------------------------------------- | ------------------------------------------- |
| Single binary, no runtime deps        | Task 1, 12                                  |
| MCP stdio transport                   | Task 10                                     |
| MCP HTTP/SSE transport                | Task 10 (stub — marked not yet implemented) |
| OpenAPI backend                       | Task 8                                      |
| GraphQL backend                       | Task 9                                      |
| Dynamic CLI generation                | Task 7                                      |
| TTL disk cache                        | Task 6                                      |
| `--pretty`, `--raw`, `--head`, `--jq` | Task 5                                      |
| `--list`, `--search`                  | Task 11                                     |
| `env:`/`file:` secret resolution      | Task 4                                      |
| Error → stderr + exit 1               | Task 11                                     |
| `--refresh` flag                      | Task 11                                     |
| `--fields` GraphQL override           | Task 9, 11                                  |

**MCP HTTP/SSE gap:** The spec says MCP HTTP/SSE is in scope. Task 10 stubs it with a `Transport` error. After the core plan is complete, MCP HTTP can be added as a follow-up task using `reqwest` SSE streaming — it follows the same `StdioSession` shape but over HTTP.

**Type consistency confirmed:** `CommandDef`, `ParamDef`, `ParamLocation`, `ArgMap`, `BackendError`, `FormatOptions` defined in Task 2/5 and used consistently across Tasks 7–11. `to_kebab` defined in `openapi.rs` and imported by `mcp.rs` and `graphql.rs`.
