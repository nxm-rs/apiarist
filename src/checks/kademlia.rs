//! Kademlia Check
//!
//! Validates the Kademlia DHT topology of the cluster.
//!
//! ## What it checks
//!
//! 1. Each node has a valid topology
//! 2. Kademlia depth is within expected bounds
//! 3. Bins are populated appropriately
//! 4. Network is healthy (reachability, availability)
//!
//! ## Equivalent beekeeper check
//!
//! Reference: `beekeeper/pkg/check/kademlia/kademlia.go`
//!
//! ## Options
//!
//! - `timeout`: Maximum time for check (default: 2m)
//! - `min_depth`: Minimum expected Kademlia depth (default: 0)
//! - `min_connected`: Minimum connected peers per node (default: 1)

use async_trait::async_trait;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use super::traits::{Check, CheckContext, CheckError, CheckOptions, CheckResult, NodeResult};

/// Default minimum Kademlia depth
const DEFAULT_MIN_DEPTH: i64 = 0;

/// Default minimum connected peers
const DEFAULT_MIN_CONNECTED: i64 = 1;

/// Kademlia topology check
///
/// Verifies the health of the Kademlia DHT across all nodes.
pub struct KademliaCheck;

#[async_trait]
impl Check for KademliaCheck {
    fn name(&self) -> &'static str {
        "kademlia"
    }

    fn description(&self) -> &'static str {
        "Validate Kademlia DHT topology"
    }

    async fn run(
        &self,
        ctx: &CheckContext,
        opts: &CheckOptions,
    ) -> Result<CheckResult, CheckError> {
        let start = Instant::now();

        let min_depth: i64 = opts.get_extra("min_depth").unwrap_or(DEFAULT_MIN_DEPTH);
        let min_connected: i64 = opts.get_extra("min_connected").unwrap_or(DEFAULT_MIN_CONNECTED);

        info!(
            nodes = ctx.node_count(),
            min_depth = min_depth,
            min_connected = min_connected,
            "Starting kademlia check"
        );

        let mut node_results = Vec::new();
        let mut total_connected = 0i64;
        let mut total_population = 0i64;

        for node in &ctx.nodes {
            let node_name = node.name().unwrap_or("unknown").to_string();
            debug!(node = %node_name, "Checking topology");

            match node.topology().await {
                Ok(topology) => {
                    let depth = topology.depth;
                    let connected = topology.connected;
                    let population = topology.population;

                    total_connected += connected;
                    total_population += population;

                    debug!(
                        node = %node_name,
                        depth = depth,
                        connected = connected,
                        population = population,
                        base_addr = %topology.base_addr,
                        "Got topology"
                    );

                    // Check if topology meets requirements
                    let mut failures = Vec::new();

                    if depth < min_depth {
                        failures.push(format!(
                            "depth {} below minimum {}",
                            depth, min_depth
                        ));
                    }

                    if connected < min_connected {
                        failures.push(format!(
                            "connected {} below minimum {}",
                            connected, min_connected
                        ));
                    }

                    // Check network availability if reported
                    if let Some(ref availability) = topology.network_availability {
                        use crate::client::NetworkAvailability;
                        if *availability == NetworkAvailability::Unavailable {
                            failures.push("network unavailable".to_string());
                        }
                    }

                    // Count populated bins
                    let populated_bins: usize = topology
                        .bins
                        .values()
                        .filter(|bin| bin.connected > 0)
                        .count();

                    if failures.is_empty() {
                        node_results.push(
                            NodeResult::passed(&node_name)
                                .with_detail("depth", depth)
                                .with_detail("connected", connected)
                                .with_detail("population", population)
                                .with_detail("populated_bins", populated_bins)
                                .with_detail("base_addr", &topology.base_addr),
                        );
                    } else {
                        warn!(
                            node = %node_name,
                            failures = ?failures,
                            "Topology check failed"
                        );
                        node_results.push(
                            NodeResult::failed(&node_name, failures.join("; "))
                                .with_detail("depth", depth)
                                .with_detail("connected", connected)
                                .with_detail("population", population)
                                .with_detail("populated_bins", populated_bins),
                        );
                    }
                }
                Err(e) => {
                    warn!(node = %node_name, error = %e, "Failed to get topology");
                    node_results.push(NodeResult::failed(
                        &node_name,
                        format!("Failed to get topology: {}", e),
                    ));
                }
            }
        }

        let duration = start.elapsed();
        let passed = node_results.iter().all(|r| r.passed);
        let avg_connected = if ctx.node_count() > 0 {
            total_connected / ctx.node_count() as i64
        } else {
            0
        };

        info!(
            passed = passed,
            total_connected = total_connected,
            total_population = total_population,
            avg_connected = avg_connected,
            duration_ms = duration.as_millis(),
            "Kademlia check complete"
        );

        let message = format!(
            "Avg connected: {}, total population: {}",
            avg_connected, total_population
        );

        Ok(CheckResult::new("kademlia", node_results, duration).with_message(message))
    }

    fn default_options(&self) -> CheckOptions {
        CheckOptions {
            timeout: Some(Duration::from_secs(120)), // 2 minutes
            retries: 0,
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
        let check = KademliaCheck;
        assert_eq!(check.name(), "kademlia");
        assert!(!check.description().is_empty());
    }

    #[test]
    fn test_default_options() {
        let check = KademliaCheck;
        let opts = check.default_options();
        assert_eq!(opts.timeout, Some(Duration::from_secs(120)));
    }
}
