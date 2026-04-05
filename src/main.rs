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
    let scan_mode = matches.get_flag("scan");
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

    let extra_headers: Vec<(String, String)> = matches
        .get_many::<String>("header")
        .unwrap_or_default()
        .filter_map(|h| {
            let (k, v) = h.split_once(':')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .collect();

    // Merge auth + extra headers
    let all_headers: Vec<(String, String)> = auth_headers.iter().cloned().chain(extra_headers).collect();

    if scan_mode {
        return run_scan().await;
    }

    // Build backend
    let backend: Box<dyn Backend> = if let Some(cmd) = matches.get_one::<String>("mcp-stdio") {
        Box::new(McpBackend::from_stdio(cmd.clone()))
    } else if let Some(url) = matches.get_one::<String>("mcp") {
        Box::new(McpBackend::from_http(url.clone(), all_headers.clone()))
    } else if let Some(url) = matches.get_one::<String>("graphql") {
        let mut b = GraphQlBackend::new(url.clone(), all_headers.clone());
        if let Some(fields) = matches.get_one::<String>("fields") {
            b = b.with_fields_override(fields.clone());
        }
        Box::new(b)
    } else if let Some(spec) = matches.get_one::<String>("spec") {
        let user_base_url = matches.get_one::<String>("base-url").cloned();
        if spec.starts_with("http://") || spec.starts_with("https://") {
            let spec_json = fetch_spec(spec, &all_headers).await?;
            let base_url = user_base_url.unwrap_or_else(|| {
                let raw = spec_json.pointer("/servers/0/url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("http://localhost");
                // Auto-resolve relative URLs against the spec's fetch URL
                if raw.starts_with("http://") || raw.starts_with("https://") {
                    raw.to_string()
                } else {
                    // Strip path from spec URL and append relative server URL
                    if let Ok(parsed) = url::Url::parse(spec) {
                        let origin = format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or("localhost"));
                        let port_str = parsed.port().map(|p| format!(":{p}")).unwrap_or_default();
                        format!("{}{}{}", origin, port_str, raw)
                    } else {
                        raw.to_string()
                    }
                }
            });
            Box::new(OpenApiBackend::from_json(spec_json, base_url, all_headers.clone()))
        } else {
            let mut b = OpenApiBackend::from_file(spec).context("loading OpenAPI spec")?;
            if let Some(bu) = user_base_url {
                b = b.with_base_url(bu);
            }
            b = b.with_auth_headers(all_headers.clone());
            Box::new(b)
        }
    } else if let Some(cli_cmd) = matches.get_one::<String>("cli") {
        use mcpipe::backend::cli::CliBackend;
        Box::new(CliBackend::new(cli_cmd.clone()))
    } else {
        bail!("specify one of --mcp-stdio, --mcp, --spec, --graphql, or --cli");
    };

    let gen_openapi = matches.get_flag("gen-openapi");
    let openapi_output = matches.get_one::<String>("openapi-output").cloned();

    if gen_openapi && matches.get_one::<String>("cli").is_none() {
        bail!("--gen-openapi requires --cli <COMMAND>");
    }

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
        let filtered: Vec<_> = cmds.iter().filter(|cmd| {
            pattern.is_empty()
                || cmd.name.to_lowercase().contains(&pattern)
                || cmd.description.to_lowercase().contains(&pattern)
        }).collect();

        let max_name = filtered.iter().map(|c| c.name.len()).max().unwrap_or(20);
        let width = max_name.max(10);
        for cmd in &filtered {
            let param_count = cmd.params.len();
            let suffix = if param_count > 0 { format!(" ({param_count} params)") } else { String::new() };
            println!("{:<width$}  {}{}",  cmd.name, cmd.description, suffix);
        }
        return Ok(());
    }

    // Build dynamic CLI from discovered commands
    let dynamic = build_command("mcpipe", &cmds);

    // Strip global flags from argv to get tool subcommand args
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let global_value_flags = [
        "--mcp-stdio", "--mcp", "--spec", "--graphql",
        "--auth-header", "--header", "--base-url", "--cache-ttl", "--jq", "--head", "--search", "--fields",
        "--cli", "--openapi-output",
    ];
    let global_bool_flags = ["--pretty", "--raw", "--refresh", "--list", "--scan", "--gen-openapi"];

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
        .arg(Arg::new("auth-header").long("auth-header").value_name("NAME:VALUE").action(ArgAction::Append).help("Auth header with secret resolution (repeatable)"))
        .arg(Arg::new("header").long("header").value_name("KEY:VALUE").action(ArgAction::Append).help("Arbitrary HTTP header (repeatable)"))
        .arg(Arg::new("base-url").long("base-url").value_name("URL").help("Override base URL for OpenAPI spec"))
        .arg(Arg::new("pretty").long("pretty").action(ArgAction::SetTrue).help("Pretty-print JSON output"))
        .arg(Arg::new("raw").long("raw").action(ArgAction::SetTrue).help("Print raw string values"))
        .arg(Arg::new("refresh").long("refresh").action(ArgAction::SetTrue).help("Bypass cache, re-fetch"))
        .arg(Arg::new("list").long("list").action(ArgAction::SetTrue).help("List available subcommands"))
        .arg(Arg::new("scan").long("scan").action(ArgAction::SetTrue)
            .help("Auto-discover all API surfaces and print a unified catalog"))
        .arg(Arg::new("search").long("search").value_name("PATTERN").help("Search commands by name/description"))
        .arg(Arg::new("cache-ttl").long("cache-ttl").value_name("SECS").value_parser(clap::value_parser!(u64)).help("Cache TTL in seconds (default: 3600)"))
        .arg(Arg::new("jq").long("jq").value_name("EXPR").help("Filter output through jq"))
        .arg(Arg::new("head").long("head").value_name("N").value_parser(clap::value_parser!(usize)).help("Limit output to first N array elements"))
        .arg(Arg::new("fields").long("fields").value_name("FIELDS").help("Override GraphQL selection set fields"))
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
        .allow_external_subcommands(true)
}

async fn run_scan() -> anyhow::Result<()> {
    use mcpipe::scanner::claude_config::ClaudeConfigScanner;
    use mcpipe::scanner::workspace::WorkspaceScanner;
    use mcpipe::scanner::well_known::WellKnownScanner;
    use mcpipe::discovery::SourceScanner;

    eprintln!("Scanning for API surfaces...");

    let scanners: Vec<Box<dyn SourceScanner>> = vec![
        Box::new(ClaudeConfigScanner::default_env()),
        Box::new(WorkspaceScanner::default_env()),
        Box::new(WellKnownScanner::new()),
    ];

    let scan_futures: Vec<_> = scanners.iter().map(|s| s.scan()).collect();
    let all_results = futures::future::join_all(scan_futures).await;

    let mut all_sources: Vec<_> = all_results.into_iter().flatten().collect();
    all_sources.sort_by(|a, b| a.name.cmp(&b.name));

    if all_sources.is_empty() {
        println!("No API surfaces found.");
        return Ok(());
    }

    eprintln!("Found {} source(s). Discovering tools...\n", all_sources.len());

    let discover_futures: Vec<_> = all_sources.iter().map(|src| {
        let backend = src.clone().into_backend();
        let name = src.name.clone();
        let origin = src.origin.clone();
        async move {
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                backend.discover(),
            ).await;
            (name, origin, result)
        }
    }).collect();

    let results = futures::future::join_all(discover_futures).await;

    let mut total = 0usize;
    let mut errors = 0usize;
    for (name, origin, result) in &results {
        match result {
            Ok(Ok(cmds)) => {
                println!("## {} ({})", name, origin);
                let max_w = cmds.iter().map(|c| c.name.len()).max().unwrap_or(10).max(10);
                for cmd in cmds {
                    // Take only the first line of multi-line descriptions and cap at 80 chars.
                    let desc = cmd.description.lines().next().unwrap_or("").trim();
                    let desc = if desc.len() > 80 { &desc[..80] } else { desc };
                    println!("  {:<width$}  {}", cmd.name, desc, width = max_w);
                }
                println!("  ({} tools)\n", cmds.len());
                total += cmds.len();
            }
            Ok(Err(e)) => {
                eprintln!("  [skip] {} — {}", name, e);
                errors += 1;
            }
            Err(_) => {
                eprintln!("  [skip] {} — timeout (>10s)", name);
                errors += 1;
            }
        }
    }

    let src_ok = results.len() - errors;
    println!("Total: {} tools across {} source(s){}.", total, src_ok,
        if errors > 0 { format!(" ({errors} skipped — see stderr)") } else { String::new() });
    Ok(())
}

async fn fetch_spec(url: &str, auth_headers: &[(String, String)]) -> Result<serde_json::Value> {
    use mcpipe::deser::{parse_any, FormatHint};

    let client = reqwest::Client::new();
    let mut req = client.get(url);
    for (k, v) in auth_headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let resp = req.send().await.context("fetching spec")?;
    if !resp.status().is_success() {
        bail!("HTTP {}: {}", resp.status(), resp.text().await.unwrap_or_default());
    }

    // Derive format hint: Content-Type header first, then URL file extension
    let ct_hint = resp.headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(FormatHint::from_content_type)
        .unwrap_or(FormatHint::Unknown);

    let url_hint = std::path::Path::new(url)
        .extension()
        .and_then(|e| e.to_str())
        .map(FormatHint::from_extension)
        .unwrap_or(FormatHint::Unknown);

    // Content-Type wins if it's definitive, otherwise fall back to URL extension
    let hint = if ct_hint != FormatHint::Unknown { ct_hint } else { url_hint };

    let bytes = resp.bytes().await.context("reading spec body")?;
    parse_any(&bytes, hint).context("parsing spec")
}
