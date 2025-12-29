//! Full Connectivity Check
//!
//! Verifies that all nodes in the cluster have appropriate connectivity
//! based on their node type:
//!
//! - **Boot nodes**: Must connect to ALL other nodes (total - 1)
//! - **Full nodes**: Must connect to all other full-capable nodes (full_capable - 1)
//! - **Light nodes**: Must have at least 1 peer
//!
//! ## What it checks
//!
//! 1. Collects peer lists from all nodes
//! 2. Validates each node has the expected peer count for its type
//! 3. Verifies all peers are valid cluster members
//!
//! ## Equivalent beekeeper check
//!
//! Reference: `beekeeper/pkg/check/fullconnectivity/fullconnectivity.go`
//!
//! ## Options
//!
//! - `timeout`: Maximum time for check (default: 10m)

use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use super::traits::{Check, CheckContext, CheckError, CheckOptions, CheckResult, NodeResult};
use crate::config::NodeType;

/// Full connectivity check
///
/// Tests that every node has the expected number of peers based on its type.
pub struct FullconnectivityCheck;

/// Results broken down by node type
#[derive(Debug, Default)]
struct ConnectivityStats {
    boot_passed: usize,
    boot_failed: usize,
    full_passed: usize,
    full_failed: usize,
    light_passed: usize,
    light_failed: usize,
}

impl ConnectivityStats {
    fn total_passed(&self) -> usize {
        self.boot_passed + self.full_passed + self.light_passed
    }

    fn total_failed(&self) -> usize {
        self.boot_failed + self.full_failed + self.light_failed
    }

    fn all_passed(&self) -> bool {
        self.total_failed() == 0
    }

    fn summary(&self) -> String {
        format!(
            "Boot: {}/{}, Full: {}/{}, Light: {}/{}",
            self.boot_passed,
            self.boot_passed + self.boot_failed,
            self.full_passed,
            self.full_passed + self.full_failed,
            self.light_passed,
            self.light_passed + self.light_failed,
        )
    }
}

#[async_trait]
impl Check for FullconnectivityCheck {
    fn name(&self) -> &'static str {
        "fullconnectivity"
    }

    fn description(&self) -> &'static str {
        "Verify all nodes have appropriate connectivity for their type"
    }

    async fn run(
        &self,
        ctx: &CheckContext,
        _opts: &CheckOptions,
    ) -> Result<CheckResult, CheckError> {
        let start = Instant::now();

        let counts = ctx.node_type_counts();
        info!(
            total_nodes = counts.total(),
            boot_nodes = counts.boot,
            full_nodes = counts.full,
            light_nodes = counts.light,
            "Starting fullconnectivity check"
        );

        // Step 1: Collect all overlay addresses
        let mut overlays: HashMap<String, String> = HashMap::new();
        let mut overlay_set: HashSet<String> = HashSet::new();

        for node in &ctx.nodes {
            let node_name = node.name().unwrap_or("unknown").to_string();
            match node.addresses().await {
                Ok(addrs) => {
                    overlays.insert(node_name.clone(), addrs.overlay.clone());
                    overlay_set.insert(addrs.overlay.clone());
                    debug!(
                        node = %node_name,
                        overlay = %addrs.overlay,
                        node_type = %node.node_type(),
                        "Got overlay address"
                    );
                }
                Err(e) => {
                    warn!(node = %node_name, error = %e, "Failed to get addresses");
                }
            }
        }

        // Step 2: Check each node's connectivity
        let mut node_results = Vec::new();
        let mut stats = ConnectivityStats::default();

        for node in &ctx.nodes {
            let node_name = node.name().unwrap_or("unknown").to_string();
            let node_type = node.node_type();
            let expected_peers = ctx.expected_peers_for(node);

            let result = match node.peers().await {
                Ok(peers) => {
                    let peer_count = peers.peers.len();

                    // Validate all peers are known cluster members
                    let invalid_peers: Vec<_> = peers
                        .peers
                        .iter()
                        .filter(|p| !overlay_set.contains(&p.address))
                        .map(|p| p.address.clone())
                        .collect();

                    let has_enough_peers = match node_type {
                        NodeType::Boot => peer_count >= expected_peers,
                        NodeType::Full => peer_count >= expected_peers,
                        NodeType::Light => peer_count >= 1,
                    };

                    let has_valid_peers = invalid_peers.is_empty();
                    let passed = has_enough_peers && has_valid_peers;

                    if passed {
                        debug!(
                            node = %node_name,
                            node_type = %node_type,
                            peers = peer_count,
                            expected = expected_peers,
                            "Node passed"
                        );

                        // Update stats
                        match node_type {
                            NodeType::Boot => stats.boot_passed += 1,
                            NodeType::Full => stats.full_passed += 1,
                            NodeType::Light => stats.light_passed += 1,
                        }

                        NodeResult::passed(&node_name)
                            .with_detail("node_type", node_type.to_string())
                            .with_detail("peer_count", peer_count)
                            .with_detail("expected_peers", expected_peers)
                            .with_detail(
                                "overlay",
                                overlays.get(&node_name).cloned().unwrap_or_default(),
                            )
                    } else {
                        let error_msg = if !has_enough_peers {
                            format!(
                                "Insufficient peers: {peer_count} < {expected_peers} (expected for {node_type})"
                            )
                        } else {
                            format!("Invalid peers detected: {invalid_peers:?}")
                        };

                        warn!(
                            node = %node_name,
                            node_type = %node_type,
                            peers = peer_count,
                            expected = expected_peers,
                            invalid_peers = ?invalid_peers,
                            "Node failed"
                        );

                        // Update stats
                        match node_type {
                            NodeType::Boot => stats.boot_failed += 1,
                            NodeType::Full => stats.full_failed += 1,
                            NodeType::Light => stats.light_failed += 1,
                        }

                        NodeResult::failed(&node_name, error_msg)
                            .with_detail("node_type", node_type.to_string())
                            .with_detail("peer_count", peer_count)
                            .with_detail("expected_peers", expected_peers)
                            .with_detail("invalid_peers", invalid_peers)
                            .with_detail(
                                "overlay",
                                overlays.get(&node_name).cloned().unwrap_or_default(),
                            )
                    }
                }
                Err(e) => {
                    warn!(node = %node_name, error = %e, "Failed to get peers");

                    // Count as failed for the node's type
                    match node_type {
                        NodeType::Boot => stats.boot_failed += 1,
                        NodeType::Full => stats.full_failed += 1,
                        NodeType::Light => stats.light_failed += 1,
                    }

                    NodeResult::failed(&node_name, format!("Failed to get peers: {e}"))
                        .with_detail("node_type", node_type.to_string())
                }
            };

            node_results.push(result);
        }

        let duration = start.elapsed();
        let passed = stats.all_passed();

        info!(
            passed = passed,
            stats = %stats.summary(),
            duration_ms = duration.as_millis(),
            "Fullconnectivity check complete"
        );

        let message = format!(
            "Connectivity: {} passed, {} failed ({})",
            stats.total_passed(),
            stats.total_failed(),
            stats.summary()
        );

        Ok(CheckResult::new("fullconnectivity", node_results, duration).with_message(message))
    }

    fn default_options(&self) -> CheckOptions {
        CheckOptions {
            timeout: Some(Duration::from_secs(600)), // 10 minutes
            retries: 3,
            retry_delay: Some(Duration::from_secs(2)),
            extra: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_metadata() {
        let check = FullconnectivityCheck;
        assert_eq!(check.name(), "fullconnectivity");
        assert!(!check.description().is_empty());
    }

    #[test]
    fn test_default_options() {
        let check = FullconnectivityCheck;
        let opts = check.default_options();
        assert_eq!(opts.timeout, Some(Duration::from_secs(600)));
        assert_eq!(opts.retries, 3);
    }

    #[test]
    fn test_connectivity_stats_summary() {
        let stats = ConnectivityStats {
            boot_passed: 1,
            boot_failed: 0,
            full_passed: 2,
            full_failed: 1,
            light_passed: 3,
            light_failed: 0,
        };
        assert_eq!(stats.summary(), "Boot: 1/1, Full: 2/3, Light: 3/3");
        assert!(!stats.all_passed());
        assert_eq!(stats.total_passed(), 6);
        assert_eq!(stats.total_failed(), 1);
    }

    #[test]
    fn test_connectivity_stats_all_passed() {
        let stats = ConnectivityStats {
            boot_passed: 1,
            boot_failed: 0,
            full_passed: 3,
            full_failed: 0,
            light_passed: 2,
            light_failed: 0,
        };
        assert!(stats.all_passed());
        assert_eq!(stats.total_passed(), 6);
        assert_eq!(stats.total_failed(), 0);
    }
}
