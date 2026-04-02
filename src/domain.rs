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
