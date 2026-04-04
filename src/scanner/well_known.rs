use async_trait::async_trait;

use crate::discovery::{BackendKind, DiscoveredSource, SourceScanner};

enum WellKnownKind {
    McpHttp(&'static str),
}

/// (name, health-check URL, kind)
const WELL_KNOWN: &[(&str, &str, WellKnownKind)] = &[
    (
        "pieces-mcp",
        "http://localhost:39300/.well-known/health",
        WellKnownKind::McpHttp("http://localhost:39300/model_context_protocol/2024-11-05/sse"),
    ),
];

/// Probes well-known local HTTP ports to detect running API servers.
/// Each entry is checked with a GET health request; reachable ones are included.
pub struct WellKnownScanner;

impl WellKnownScanner {
    pub fn new() -> Self { Self }
}

impl Default for WellKnownScanner {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl SourceScanner for WellKnownScanner {
    async fn scan(&self) -> Vec<DiscoveredSource> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_default();

        let mut sources = vec![];

        for (name, health_url, kind) in WELL_KNOWN {
            let reachable = client.get(*health_url).send().await
                .map(|r| r.status().is_success())
                .unwrap_or(false);

            if reachable {
                let backend_kind = match kind {
                    WellKnownKind::McpHttp(url) =>
                        BackendKind::McpHttp { url: url.to_string() },
                };
                sources.push(DiscoveredSource {
                    name: name.to_string(),
                    kind: backend_kind,
                    origin: "well-known probe".to_string(),
                });
            }
        }

        sources
    }
}
