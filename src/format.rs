use anyhow::{Result, bail};
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
    child
        .stdin
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("jq stdin not available"))?
        .write_all(json.as_bytes())?;
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
        FormatOptions {
            pretty: false,
            raw: false,
            jq: None,
            head: None,
        }
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
        let out = format_value(
            &v,
            &FormatOptions {
                pretty: true,
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains('\n'));
    }

    #[test]
    fn raw_string() {
        let v = json!("hello");
        let out = format_value(
            &v,
            &FormatOptions {
                raw: true,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, "hello");
    }

    #[test]
    fn head_truncates_array() {
        let v = json!([1, 2, 3, 4, 5]);
        let out = format_value(
            &v,
            &FormatOptions {
                head: Some(3),
                ..opts()
            },
        )
        .unwrap();
        let back: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(back, json!([1, 2, 3]));
    }

    #[test]
    fn head_noop_on_object() {
        let v = json!({"a": 1});
        let out = format_value(
            &v,
            &FormatOptions {
                head: Some(1),
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&out).unwrap(), v);
    }
}
