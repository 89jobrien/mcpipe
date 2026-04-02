use anyhow::{Context, Result};
use async_trait::async_trait;

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

                let fallback_id = format!("{}-{}", method, path.trim_matches('/'));
                let operation_id = op.get("operationId")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&fallback_id);

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
                    let schema = body
                        .pointer("/content/application~1json/schema")
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

        resp.json().await.map_err(|e| BackendError::Execution(e.to_string()))
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
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('-');
        }
        out.push(c.to_lowercase().next().unwrap());
    }
    out.replace('_', "-")
}
