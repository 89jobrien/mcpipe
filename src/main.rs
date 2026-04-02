use anyhow::{bail, Context, Result};
use clap::{Arg, ArgAction, Command};
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

    let pretty = matches.get_flag("pretty");
    let raw = matches.get_flag("raw");
    let jq = matches.get_one::<String>("jq").cloned();
    let head = matches.get_one::<usize>("head").copied();
    let list_only = matches.get_flag("list");
    let search = matches.get_one::<String>("search").cloned();
    let refresh = matches.get_flag("refresh");
    let cache_ttl = *matches.get_one::<u64>("cache-ttl").unwrap_or(&3600);

    let auth_headers: Vec<(String, String)> = matches
        .get_many::<String>("auth-header")
        .unwrap_or_default()
        .filter_map(|h| {
            let (k, v) = h.split_once(':')?;
            match resolve_secret(v.trim()) {
                Ok(resolved) => Some((k.trim().to_string(), resolved)),
                Err(e) => {
                    eprintln!("Warning: skipping auth header {k:?}: {e}");
                    None
                }
            }
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

    // Cache source key (only for non-stdio backends)
    let cache_source = matches.get_one::<String>("mcp")
        .or_else(|| matches.get_one::<String>("spec"))
        .or_else(|| matches.get_one::<String>("graphql"))
        .cloned();

    let cache = Cache::new(Cache::default_dir(), Duration::from_secs(cache_ttl));

    // Discover (with optional cache)
    let cmds = if let Some(ref source) = cache_source {
        if !refresh {
            if let Some(cached) = cache.load(source) {
                cached
            } else {
                let discovered = backend.discover().await.map_err(|e| anyhow::anyhow!("{e}"))?;
                let _ = cache.save(source, &discovered);
                discovered
            }
        } else {
            let discovered = backend.discover().await.map_err(|e| anyhow::anyhow!("{e}"))?;
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
            if pattern.is_empty()
                || cmd.name.contains(&pattern)
                || cmd.description.to_lowercase().contains(&pattern)
            {
                println!("{:30}  {}", cmd.name, cmd.description);
            }
        }
        return Ok(());
    }

    // Build dynamic CLI from discovered commands
    let dynamic = build_command("mcpipe", &cmds);

    // Strip global flags from argv to get tool subcommand args
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let global_value_flags = [
        "--mcp-stdio", "--mcp", "--spec", "--graphql",
        "--auth-header", "--cache-ttl", "--jq", "--head", "--search", "--fields",
    ];
    let global_bool_flags = ["--pretty", "--raw", "--refresh", "--list"];

    let mut tool_args = vec!["mcpipe".to_string()];
    let mut skip_next = false;
    for arg in &raw_args {
        if skip_next {
            skip_next = false;
            continue;
        }
        // Handle --flag=value form
        let flag_name = arg.split('=').next().unwrap_or(arg);
        if global_value_flags.contains(&flag_name) {
            if !arg.contains('=') {
                skip_next = true;
            }
            continue;
        }
        if global_bool_flags.contains(&arg.as_str()) {
            continue;
        }
        tool_args.push(arg.clone());
    }

    let dynamic_matches = dynamic.try_get_matches_from(&tool_args)
        .map_err(|e| { let _ = e.print(); anyhow::anyhow!("") })?;

    let (sub_name, sub_matches) = dynamic_matches.subcommand()
        .ok_or_else(|| anyhow::anyhow!("no subcommand — use --list to see available commands"))?;

    let cmd_def = cmds.iter().find(|c| c.name == sub_name)
        .ok_or_else(|| anyhow::anyhow!("unknown command: {sub_name}"))?;

    let args = extract_args(sub_matches, cmd_def);
    let result = backend.execute(cmd_def, args).await.map_err(|e| anyhow::anyhow!("{e}"))?;

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
        .arg(Arg::new("auth-header").long("auth-header").value_name("NAME:VALUE").action(ArgAction::Append).help("Auth header (repeatable)"))
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
