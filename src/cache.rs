use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::domain::CommandDef;

pub struct Cache {
    dir: PathBuf,
    ttl: Duration,
}

impl Cache {
    pub fn new(dir: PathBuf, ttl: Duration) -> Self {
        Self { dir, ttl }
    }

    pub fn default_dir() -> PathBuf {
        std::env::var("MCPIPE_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::cache_dir()
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join("mcpipe")
            })
    }

    fn key(source: &str) -> String {
        let hash = Sha256::digest(source.as_bytes());
        hex::encode(&hash[..8]) // 16 hex chars
    }

    fn path(&self, source: &str) -> PathBuf {
        self.dir.join(format!("{}.json", Self::key(source)))
    }

    pub fn load(&self, source: &str) -> Option<Vec<CommandDef>> {
        let path = self.path(source);
        let meta = std::fs::metadata(&path).ok()?;
        let modified = meta.modified().ok()?;
        let age = SystemTime::now().duration_since(modified).ok()?;
        if age >= self.ttl {
            return None;
        }
        let data = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn save(&self, source: &str, cmds: &[CommandDef]) -> Result<()> {
        std::fs::create_dir_all(&self.dir)
            .context("creating cache dir")?;
        let path = self.path(source);
        let data = serde_json::to_string(cmds)?;
        std::fs::write(&path, data).context("writing cache file")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::CommandDef;

    fn sample_cmd() -> CommandDef {
        CommandDef {
            name: "list-pets".to_string(),
            description: "List pets".to_string(),
            source_name: "listPets".to_string(),
            params: vec![],
        }
    }

    #[test]
    fn cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().to_path_buf(), Duration::from_secs(3600));
        let cmds = vec![sample_cmd()];
        cache.save("http://example.com/spec", &cmds).unwrap();
        let loaded = cache.load("http://example.com/spec").unwrap();
        assert_eq!(loaded[0].name, "list-pets");
    }

    #[test]
    fn cache_miss_on_different_source() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().to_path_buf(), Duration::from_secs(3600));
        cache.save("source-a", &[sample_cmd()]).unwrap();
        assert!(cache.load("source-b").is_none());
    }

    #[test]
    fn cache_expired_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        // TTL of 0 seconds — always expired
        let cache = Cache::new(dir.path().to_path_buf(), Duration::from_secs(0));
        cache.save("source", &[sample_cmd()]).unwrap();
        // sleep 10ms so modified time is strictly in the past
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(cache.load("source").is_none());
    }
}
