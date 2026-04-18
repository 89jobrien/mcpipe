use anyhow::{Context, Result};
use std::path::Path;

/// Resolve a secret value.
/// - `env:VAR` → read from environment variable
/// - `file:/path` → read file, strip trailing newline (LF or CRLF)
/// - anything else → return as-is
pub fn resolve_secret(value: &str) -> Result<String> {
    if let Some(var) = value.strip_prefix("env:") {
        std::env::var(var).with_context(|| format!("env var {var:?} is not set"))
    } else if let Some(path) = value.strip_prefix("file:") {
        let content = std::fs::read_to_string(Path::new(path))
            .with_context(|| format!("reading secret file {path:?}"))?;
        Ok(content.trim_end_matches(&['\n', '\r'][..]).to_string())
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
        assert_eq!(
            resolve_secret("env:MCPIPE_TEST_SECRET").unwrap(),
            "secret123"
        );
    }

    #[test]
    fn env_missing_errors() {
        unsafe { std::env::remove_var("MCPIPE_TEST_MISSING") };
        assert!(resolve_secret("env:MCPIPE_TEST_MISSING").is_err());
    }

    #[test]
    fn file_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.txt");
        std::fs::write(&path, "filetoken\n").unwrap();
        let spec = format!("file:{}", path.display());
        assert_eq!(resolve_secret(&spec).unwrap(), "filetoken");
    }

    #[test]
    fn file_prefix_crlf() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.txt");
        std::fs::write(&path, "filetoken\r\n").unwrap();
        let spec = format!("file:{}", path.display());
        assert_eq!(resolve_secret(&spec).unwrap(), "filetoken");
    }
}
