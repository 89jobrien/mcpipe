use anyhow::{Context, Result};

/// Format hint derived from file extension or Content-Type header.
#[derive(Debug, Clone, PartialEq)]
pub enum FormatHint {
    Json,
    Yaml,
    Toml,
    Json5,
    Unknown,
}

impl FormatHint {
    /// Derive from a file extension (e.g. "yaml", "json").
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "json" => Self::Json,
            "yaml" | "yml" => Self::Yaml,
            "toml" => Self::Toml,
            "json5" => Self::Json5,
            _ => Self::Unknown,
        }
    }

    /// Derive from a Content-Type header value.
    pub fn from_content_type(ct: &str) -> Self {
        let ct = ct.to_lowercase();
        if ct.contains("yaml") {
            Self::Yaml
        } else if ct.contains("toml") {
            Self::Toml
        } else if ct.contains("json") {
            Self::Json
        } else {
            Self::Unknown
        }
    }
}

/// Parse bytes in any supported format into a `serde_json::Value`.
///
/// If `hint` is `Unknown`, tries JSON first, then YAML, then TOML, then JSON5.
pub fn parse_any(bytes: &[u8], hint: FormatHint) -> Result<serde_json::Value> {
    let text = std::str::from_utf8(bytes).context("spec is not valid UTF-8")?;

    match hint {
        FormatHint::Json  => parse_json(text),
        FormatHint::Yaml  => parse_yaml(text),
        FormatHint::Toml  => parse_toml(text),
        FormatHint::Json5 => parse_json5(text),
        FormatHint::Unknown => {
            // Try formats in order of prevalence
            parse_json(text)
                .or_else(|_| parse_yaml(text))
                .or_else(|_| parse_toml(text))
                .or_else(|_| parse_json5(text))
                .context("could not parse spec as JSON, YAML, TOML, or JSON5")
        }
    }
}

fn parse_json(text: &str) -> Result<serde_json::Value> {
    serde_json::from_str(text).context("JSON parse error")
}

fn parse_yaml(text: &str) -> Result<serde_json::Value> {
    serde_yaml::from_str(text).context("YAML parse error")
}

fn parse_toml(text: &str) -> Result<serde_json::Value> {
    let toml_val: toml::Value = toml::from_str(text).context("TOML parse error")?;
    // Convert toml::Value → serde_json::Value via JSON round-trip
    let json_str = serde_json::to_string(&toml_val).context("TOML→JSON conversion")?;
    serde_json::from_str(&json_str).context("TOML→JSON parse")
}

fn parse_json5(text: &str) -> Result<serde_json::Value> {
    json5::from_str(text).context("JSON5 parse error")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_object() {
        let v = parse_any(br#"{"a": 1}"#, FormatHint::Json).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn parse_yaml_object() {
        let v = parse_any(b"a: 1\nb: hello\n", FormatHint::Yaml).unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"], "hello");
    }

    #[test]
    fn parse_toml_object() {
        let v = parse_any(b"[package]\nname = \"foo\"\n", FormatHint::Toml).unwrap();
        assert_eq!(v["package"]["name"], "foo");
    }

    #[test]
    fn parse_json5_with_comments() {
        let v = parse_any(b"{ /* comment */ a: 1 }", FormatHint::Json5).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn unknown_hint_tries_all() {
        // Valid YAML that is not valid JSON
        let v = parse_any(b"key: value\n", FormatHint::Unknown).unwrap();
        assert_eq!(v["key"], "value");
    }

    #[test]
    fn hint_from_extension() {
        assert_eq!(FormatHint::from_extension("yaml"), FormatHint::Yaml);
        assert_eq!(FormatHint::from_extension("yml"), FormatHint::Yaml);
        assert_eq!(FormatHint::from_extension("json"), FormatHint::Json);
        assert_eq!(FormatHint::from_extension("toml"), FormatHint::Toml);
        assert_eq!(FormatHint::from_extension("json5"), FormatHint::Json5);
        assert_eq!(FormatHint::from_extension("xyz"), FormatHint::Unknown);
    }

    #[test]
    fn hint_from_content_type() {
        assert_eq!(FormatHint::from_content_type("application/yaml"), FormatHint::Yaml);
        assert_eq!(FormatHint::from_content_type("application/json"), FormatHint::Json);
        assert_eq!(FormatHint::from_content_type("text/plain"), FormatHint::Unknown);
    }
}
