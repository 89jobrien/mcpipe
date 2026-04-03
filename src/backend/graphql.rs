use async_trait::async_trait;

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
        const INTROSPECTION_QUERY: &str = r#"{ __schema { queryType { name } mutationType { name } types { name fields(includeDeprecated: false) { name description args { name description type { kind name ofType { kind name ofType { kind name } } } defaultValue } type { kind name ofType { kind name ofType { kind name } } } } } } }"#;

        let client = reqwest::Client::new();
        let mut req = client.post(&self.endpoint)
            .json(&serde_json::json!({"query": INTROSPECTION_QUERY}));
        for (k, v) in &self.auth_headers {
            req = req.header(k, v);
        }
        let resp = req.send().await
            .map_err(|e| BackendError::Transport(e.to_string()))?;
        resp.json().await.map_err(|e| BackendError::Schema(e.to_string()))
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
            let is_mutation = mutation_type.is_some_and(|mt| type_name == mt);

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
        // Extract per-subcommand fields override (not a real GraphQL arg)
        let per_call_fields = args.get("__fields").and_then(|v| v.as_str()).map(|s| s.to_string());

        let arg_str: String = args.iter()
            .filter(|(k, _)| k.as_str() != "__fields")
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect::<Vec<_>>()
            .join(", ");

        let call = if arg_str.is_empty() {
            cmd.source_name.clone()
        } else {
            format!("{}({})", cmd.source_name, arg_str)
        };

        let fields = per_call_fields
            .or_else(|| self.fields_override.clone())
            .unwrap_or_else(|| "id".to_string());
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

        Ok(val.pointer(&format!("/data/{}", cmd.source_name)).cloned().unwrap_or(val))
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
