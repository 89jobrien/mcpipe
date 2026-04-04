// stub — implemented in Task 4
use async_trait::async_trait;
use crate::discovery::{DiscoveredSource, SourceScanner};

pub struct WellKnownScanner;

impl WellKnownScanner {
    pub fn new() -> Self { Self }
}

impl Default for WellKnownScanner {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl SourceScanner for WellKnownScanner {
    async fn scan(&self) -> Vec<DiscoveredSource> { vec![] }
}
