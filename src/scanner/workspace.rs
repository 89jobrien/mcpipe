// stub — implemented in Task 3
use async_trait::async_trait;
use crate::discovery::{DiscoveredSource, SourceScanner};

pub struct WorkspaceScanner {
    pub roots: Vec<String>,
}

impl WorkspaceScanner {
    pub fn from_roots(roots: Vec<String>) -> Self { Self { roots } }
    pub fn default_env() -> Self { Self { roots: vec![] } }
}

#[async_trait]
impl SourceScanner for WorkspaceScanner {
    async fn scan(&self) -> Vec<DiscoveredSource> { vec![] }
}
