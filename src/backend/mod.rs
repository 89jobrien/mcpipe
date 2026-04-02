pub mod mcp;
pub mod openapi;
pub mod graphql;

use async_trait::async_trait;
use crate::domain::{ArgMap, BackendError, CommandDef};

#[async_trait]
pub trait Backend: Send + Sync {
    async fn discover(&self) -> Result<Vec<CommandDef>, BackendError>;
    async fn execute(
        &self,
        cmd: &CommandDef,
        args: ArgMap,
    ) -> Result<serde_json::Value, BackendError>;
}
