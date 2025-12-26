//! Full Connectivity Check
//!
//! Verifies that all nodes in the cluster can communicate with each other.
//! This is more thorough than pingpong as it tests every node pair.
//!
//! ## What it checks
//!
//! 1. Every node can ping every other node
//! 2. All ping operations complete successfully
//! 3. Measures connectivity matrix
//!
//! ## Equivalent beekeeper check
//!
//! Reference: `beekeeper/pkg/check/fullconnectivity/fullconnectivity.go`
//!
//! ## Options
//!
//! - `timeout`: Maximum time for check (default: 10m)
//! - `retries`: Number of retry attempts per ping (default: 3)
//! - `retry_delay`: Delay between retries (default: 2s)

use async_trait::async_trait;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use super::traits::{Check, CheckContext, CheckError, CheckOptions, CheckResult, NodeResult};

/// Full connectivity check
///
/// Tests that every node can reach every other node in the cluster.
pub struct FullconnectivityCheck;

#[async_trait]
impl Check for FullconnectivityCheck {
    fn name(&self) -> &'static str {
        "fullconnectivity"
    }

    fn description(&self) -> &'static str {
        "Verify all nodes can communicate with each other"
    }

    async fn run(
        &self,
        ctx: &CheckContext,
        opts: &CheckOptions,
    ) -> Result<CheckResult, CheckError> {
        let start = Instant::now();
        let max_retries = opts.retries.max(1);
        let retry_delay = opts.retry_delay_or(Duration::from_secs(2));

        let node_count = ctx.node_count();
        let expected_connections = if node_count > 1 {
            node_count * (node_count - 1)
        } else {
            0
        };

        info!(
            nodes = node_count,
            expected_connections = expected_connections,
            retries = max_retries,
            "Starting fullconnectivity check"
        );

        // First, collect all overlay addresses
        let mut overlays: HashMap<String, String> = HashMap::new();
        for node in &ctx.nodes {
            let node_name = node.name().unwrap_or("unknown").to_string();
            match node.addresses().await {
                Ok(addrs) => {
                    overlays.insert(node_name.clone(), addrs.overlay.clone());
                    debug!(node = %node_name, overlay = %addrs.overlay, "Got overlay address");
                }
                Err(e) => {
                    warn!(node = %node_name, error = %e, "Failed to get addresses");
                }
            }
        }

        let mut node_results = Vec::new();
        let mut successful_pings = 0usize;
        let mut failed_pings = 0usize;

        // Test connectivity from each node to all others
        for source_node in &ctx.nodes {
            let source_name = source_node.name().unwrap_or("unknown").to_string();
            let source_overlay = overlays.get(&source_name).cloned();

            let mut node_passed = true;
            let mut ping_results = Vec::new();
            let mut success_count = 0usize;
            let mut fail_count = 0usize;

            // Ping all other nodes
            for (target_name, target_overlay) in &overlays {
                // Skip self
                if Some(target_overlay) == source_overlay.as_ref() {
                    continue;
                }

                let mut success = false;

                for attempt in 0..max_retries {
                    if attempt > 0 {
                        let delay = retry_delay * attempt;
                        debug!(
                            source = %source_name,
                            target = %target_name,
                            attempt = attempt,
                            "Retrying ping"
                        );
                        tokio::time::sleep(delay).await;
                    }

                    match source_node.pingpong(target_overlay).await {
                        Ok(response) => {
                            debug!(
                                source = %source_name,
                                target = %target_name,
                                rtt = %response.rtt,
                                "Ping successful"
                            );
                            ping_results.push(serde_json::json!({
                                "target": target_name,
                                "target_overlay": target_overlay,
                                "rtt": response.rtt,
                                "attempts": attempt + 1,
                                "success": true
                            }));
                            success = true;
                            success_count += 1;
                            break;
                        }
                        Err(e) => {
                            if attempt == max_retries - 1 {
                                warn!(
                                    source = %source_name,
                                    target = %target_name,
                                    error = %e,
                                    attempts = max_retries,
                                    "Ping failed after all retries"
                                );
                            }
                        }
                    }
                }

                if !success {
                    node_passed = false;
                    fail_count += 1;
                    ping_results.push(serde_json::json!({
                        "target": target_name,
                        "target_overlay": target_overlay,
                        "attempts": max_retries,
                        "success": false
                    }));
                }
            }

            successful_pings += success_count;
            failed_pings += fail_count;

            let result = if node_passed {
                NodeResult::passed(&source_name)
                    .with_detail("overlay", source_overlay.as_deref().unwrap_or("unknown"))
                    .with_detail("successful_pings", success_count)
                    .with_detail("failed_pings", fail_count)
                    .with_detail("pings", ping_results)
            } else {
                NodeResult::failed(
                    &source_name,
                    format!("Failed to reach {} node(s)", fail_count),
                )
                .with_detail("overlay", source_overlay.as_deref().unwrap_or("unknown"))
                .with_detail("successful_pings", success_count)
                .with_detail("failed_pings", fail_count)
                .with_detail("pings", ping_results)
            };

            node_results.push(result);
        }

        let duration = start.elapsed();
        let passed = node_results.iter().all(|r| r.passed);
        let total_pings = successful_pings + failed_pings;

        info!(
            passed = passed,
            successful_pings = successful_pings,
            failed_pings = failed_pings,
            total_pings = total_pings,
            expected = expected_connections,
            duration_ms = duration.as_millis(),
            "Fullconnectivity check complete"
        );

        let connectivity_pct = if expected_connections > 0 {
            (successful_pings as f64 / expected_connections as f64 * 100.0) as u32
        } else {
            100
        };

        let message = format!(
            "Connectivity: {}/{} ({}%)",
            successful_pings, expected_connections, connectivity_pct
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
}
