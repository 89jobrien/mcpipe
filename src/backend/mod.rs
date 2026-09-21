//! Defines the backend interface and protocol-specific adapters.

pub mod cli;
pub mod graphql;
pub mod mcp;
pub mod openapi;

use crate::domain::{ArgMap, BackendError, CommandDef};
use async_trait::async_trait;

#[async_trait]
pub trait Backend: Send + Sync {
    /// Returns the commands exposed by the backing source.
    async fn discover(&self) -> Result<Vec<CommandDef>, BackendError>;
    /// Executes a discovered command with its source parameter names.
    async fn execute(
        &self,
        cmd: &CommandDef,
        args: ArgMap,
    ) -> Result<serde_json::Value, BackendError>;
}
