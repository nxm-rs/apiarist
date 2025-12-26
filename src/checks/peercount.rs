//! Peercount Check
//!
//! Validates that all nodes have the expected number of connected peers.
//!
//! ## What it checks
//!
//! 1. Each node has at least the minimum required peers
//! 2. Each node can retrieve its peer list
//! 3. Reports peer counts for monitoring
//!
//! ## Equivalent beekeeper check
//!
//! Reference: `beekeeper/pkg/check/peercount/peercount.go`
//!
//! ## Options
//!
//! - `timeout`: Maximum time for check (default: 2m)
//! - `min_peers`: Minimum required peer count per node (default: 1)

use async_trait::async_trait;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use super::traits::{Check, CheckContext, CheckError, CheckOptions, CheckResult, NodeResult};

/// Default minimum peer count
const DEFAULT_MIN_PEERS: usize = 1;

/// Peercount validation check
///
/// Verifies that all nodes have sufficient connected peers.
pub struct PeercountCheck;

#[async_trait]
impl Check for PeercountCheck {
    fn name(&self) -> &'static str {
        "peercount"
    }

    fn description(&self) -> &'static str {
        "Validate peer counts across all nodes"
    }

    async fn run(
        &self,
        ctx: &CheckContext,
        opts: &CheckOptions,
    ) -> Result<CheckResult, CheckError> {
        let start = Instant::now();

        // Get minimum peer count from options
        let min_peers: usize = opts.get_extra("min_peers").unwrap_or(DEFAULT_MIN_PEERS);

        info!(
            nodes = ctx.node_count(),
            min_peers = min_peers,
            "Starting peercount check"
        );

        let mut node_results = Vec::new();
        let mut total_peers = 0usize;

        for node in &ctx.nodes {
            let node_name = node.name().unwrap_or("unknown").to_string();
            debug!(node = %node_name, "Checking peer count");

            match node.peers().await {
                Ok(peers) => {
                    let count = peers.peers.len();
                    total_peers += count;

                    debug!(node = %node_name, peer_count = count, "Got peer count");

                    if count >= min_peers {
                        node_results.push(
                            NodeResult::passed(&node_name)
                                .with_detail("peer_count", count)
                                .with_detail("min_required", min_peers),
                        );
                    } else {
                        warn!(
                            node = %node_name,
                            peer_count = count,
                            min_required = min_peers,
                            "Insufficient peers"
                        );
                        node_results.push(
                            NodeResult::failed(
                                &node_name,
                                format!("Expected {} peers, got {}", min_peers, count),
                            )
                            .with_detail("peer_count", count)
                            .with_detail("min_required", min_peers),
                        );
                    }
                }
                Err(e) => {
                    warn!(node = %node_name, error = %e, "Failed to get peers");
                    node_results.push(NodeResult::failed(
                        &node_name,
                        format!("Failed to get peers: {}", e),
                    ));
                }
            }
        }

        let duration = start.elapsed();
        let passed = node_results.iter().all(|r| r.passed);
        let avg_peers = if ctx.node_count() > 0 {
            total_peers / ctx.node_count()
        } else {
            0
        };

        info!(
            passed = passed,
            total_peers = total_peers,
            avg_peers = avg_peers,
            duration_ms = duration.as_millis(),
            "Peercount check complete"
        );

        let message = format!(
            "Total peers: {}, average: {} per node (min required: {})",
            total_peers, avg_peers, min_peers
        );

        Ok(CheckResult::new("peercount", node_results, duration).with_message(message))
    }

    fn default_options(&self) -> CheckOptions {
        CheckOptions {
            timeout: Some(Duration::from_secs(120)), // 2 minutes
            retries: 0, // No retries needed for simple peer count
            retry_delay: None,
            extra: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_metadata() {
        let check = PeercountCheck;
        assert_eq!(check.name(), "peercount");
        assert!(!check.description().is_empty());
    }

    #[test]
    fn test_default_options() {
        let check = PeercountCheck;
        let opts = check.default_options();
        assert_eq!(opts.timeout, Some(Duration::from_secs(120)));
        assert_eq!(opts.retries, 0);
    }
}
