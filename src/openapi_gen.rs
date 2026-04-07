use crate::domain::CommandDef;

/// Generate an OpenAPI 3.1 document from a list of CommandDefs.
/// Commands with any required param → POST (request body).
/// Commands with only optional params → GET (query params).
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
    let parameters: Vec<serde_json::Value> = cmd
        .params
        .iter()
        .map(|p| {
            serde_json::json!({
                "name": p.name,
                "in": "query",
                "required": p.required,
                "description": p.description,
                "schema": p.schema,
            })
        })
        .collect();

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
        let mut prop = serde_json::Map::new();
        prop.insert(
            "description".to_string(),
            serde_json::Value::String(p.description.clone()),
        );
        if let serde_json::Value::Object(schema_fields) = &p.schema {
            for (k, v) in schema_fields {
                prop.insert(k.clone(), v.clone());
            }
        }
        properties.insert(p.name.clone(), serde_json::Value::Object(prop));
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
                params: vec![ParamDef {
                    name: "status".to_string(),
                    original_name: "status".to_string(),
                    required: false,
                    description: "Filter by status".to_string(),
                    location: ParamLocation::ToolInput,
                    schema: serde_json::json!({"type": "string"}),
                }],
            },
            CommandDef {
                name: "todo-add".to_string(),
                description: "Add todos".to_string(),
                source_name: "doob".to_string(),
                params: vec![ParamDef {
                    name: "content".to_string(),
                    original_name: "content".to_string(),
                    required: true,
                    description: "Task description".to_string(),
                    location: ParamLocation::ToolInput,
                    schema: serde_json::json!({"type": "string"}),
                }],
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
        assert!(
            path["post"].is_object(),
            "todo-add (has required param) should be POST"
        );
        let body = &path["post"]["requestBody"]["content"]["application/json"]["schema"];
        assert_eq!(body["properties"]["content"]["type"], "string");
        assert!(body["properties"]["content"]["description"].is_string());
        // Must NOT have a nested "schema" key:
        assert!(body["properties"]["content"].get("schema").is_none());
    }

    #[test]
    fn yaml_output_is_valid() {
        let doc = generate("doob", "0.1.0", &sample_commands());
        let yaml = to_yaml(&doc).unwrap();
        assert!(yaml.contains("openapi: 3.1.0"));
    }
}
