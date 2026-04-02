use clap::{Arg, ArgMatches, Command};
use crate::domain::{ArgMap, CommandDef, ParamDef};

/// Build a clap Command tree from a list of CommandDefs.
/// The returned Command has one subcommand per CommandDef.
pub fn build_command(app_name: &str, cmds: &[CommandDef]) -> Command {
    // clap 4.6 requires 'static for Command/Arg IDs; leak to satisfy the bound
    let app_name_static: &'static str = Box::leak(app_name.to_string().into_boxed_str());
    let mut app = Command::new(app_name_static)
        .subcommand_required(false)
        .arg_required_else_help(false);

    for cmd in cmds {
        let name_static: &'static str = Box::leak(cmd.name.clone().into_boxed_str());
        let mut sub = Command::new(name_static)
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

    // clap 4.6 requires 'static for Arg IDs; leak to satisfy the bound
    let name_static: &'static str = Box::leak(param.name.clone().into_boxed_str());
    let mut arg = Arg::new(name_static)
        .long(name_static)
        .help(param.description.clone());

    if is_bool {
        arg = arg.action(clap::ArgAction::SetTrue);
    } else {
        let value_name_static: &'static str =
            Box::leak(param.name.to_uppercase().replace('-', "_").into_boxed_str());
        arg = arg.value_name(value_name_static);
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
