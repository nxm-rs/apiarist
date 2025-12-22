//! Pingpong Check
//!
//! Tests connectivity between Bee nodes by pinging all peers of each node.
//!
//! ## What it checks
//!
//! 1. Each node has discovered peers
//! 2. Each node can ping all its peers
//! 3. Measures round-trip time (RTT) for each ping
//!
//! ## Equivalent beekeeper check
//!
//! Reference: `beekeeper/pkg/check/pingpong/pingpong.go`
//!
//! ## Options
//!
//! - `timeout`: Maximum time for check (default: 5m)
//! - `retries`: Number of retry attempts per ping (default: 5)
//! - `retry_delay`: Base delay between retries, exponential backoff (default: 2s)

use async_trait::async_trait;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use super::traits::{Check, CheckContext, CheckError, CheckOptions, CheckResult, NodeResult};

/// Pingpong connectivity check
///
/// Verifies that all nodes can ping their peers and measures RTT.
pub struct PingpongCheck;

#[async_trait]
impl Check for PingpongCheck {
    fn name(&self) -> &'static str {
        "pingpong"
    }

    fn description(&self) -> &'static str {
        "Test peer connectivity with RTT measurement"
    }

    async fn run(&self, ctx: &CheckContext, opts: &CheckOptions) -> Result<CheckResult, CheckError> {
        let start = Instant::now();
        let max_retries = opts.retries.max(1);
        let retry_delay = opts.retry_delay_or(Duration::from_secs(2));

        info!(
            nodes = ctx.node_count(),
            retries = max_retries,
            "Starting pingpong check"
        );

        let mut node_results = Vec::new();
        let mut total_pings = 0u64;
        let mut successful_pings = 0u64;

        for node in &ctx.nodes {
            let node_name = node.name().unwrap_or("unknown").to_string();
            debug!(node = %node_name, "Testing node");

            // Get node's overlay address for logging
            let overlay = match node.addresses().await {
                Ok(addrs) => addrs.overlay,
                Err(e) => {
                    warn!(node = %node_name, error = %e, "Failed to get node addresses");
                    node_results.push(NodeResult::failed(&node_name, format!("Failed to get addresses: {}", e)));
                    continue;
                }
            };

            // Get peers
            let peers = match node.peers().await {
                Ok(p) => p,
                Err(e) => {
                    warn!(node = %node_name, error = %e, "Failed to get peers");
                    node_results.push(NodeResult::failed(&node_name, format!("Failed to get peers: {}", e)));
                    continue;
                }
            };

            if peers.peers.is_empty() {
                warn!(node = %node_name, "Node has no peers");
                node_results.push(NodeResult::failed(&node_name, "No peers found"));
                continue;
            }

            debug!(
                node = %node_name,
                overlay = %overlay,
                peer_count = peers.peers.len(),
                "Found peers"
            );

            let mut node_passed = true;
            let mut ping_results = Vec::new();

            // Ping each peer with retry logic
            for peer in &peers.peers {
                total_pings += 1;
                let peer_addr = &peer.address;
                let mut success = false;

                for attempt in 0..max_retries {
                    // Exponential backoff: 0, 2s, 4s, 6s, 8s...
                    if attempt > 0 {
                        let delay = retry_delay * attempt;
                        debug!(
                            node = %node_name,
                            peer = %peer_addr,
                            attempt = attempt,
                            delay_ms = delay.as_millis(),
                            "Retrying ping"
                        );
                        tokio::time::sleep(delay).await;
                    }

                    match node.pingpong(peer_addr).await {
                        Ok(response) => {
                            debug!(
                                node = %node_name,
                                peer = %peer_addr,
                                rtt = %response.rtt,
                                "Ping successful"
                            );
                            ping_results.push(serde_json::json!({
                                "peer": peer_addr,
                                "rtt": response.rtt,
                                "attempts": attempt + 1
                            }));
                            successful_pings += 1;
                            success = true;
                            break;
                        }
                        Err(e) => {
                            if attempt == max_retries - 1 {
                                warn!(
                                    node = %node_name,
                                    peer = %peer_addr,
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
                    ping_results.push(serde_json::json!({
                        "peer": peer_addr,
                        "error": "Failed after retries",
                        "attempts": max_retries
                    }));
                }
            }

            let result = if node_passed {
                NodeResult::passed(&node_name)
                    .with_detail("overlay", &overlay)
                    .with_detail("peer_count", peers.peers.len())
                    .with_detail("pings", ping_results)
            } else {
                NodeResult::failed(&node_name, "Some peers unreachable")
                    .with_detail("overlay", &overlay)
                    .with_detail("peer_count", peers.peers.len())
                    .with_detail("pings", ping_results)
            };

            node_results.push(result);
        }

        let duration = start.elapsed();
        let passed = node_results.iter().all(|r| r.passed);

        info!(
            passed = passed,
            total_pings = total_pings,
            successful_pings = successful_pings,
            duration_ms = duration.as_millis(),
            "Pingpong check complete"
        );

        let message = format!(
            "{}/{} pings successful across {} nodes",
            successful_pings,
            total_pings,
            ctx.node_count()
        );

        Ok(CheckResult::new("pingpong", node_results, duration).with_message(message))
    }

    fn default_options(&self) -> CheckOptions {
        CheckOptions {
            timeout: Some(Duration::from_secs(300)), // 5 minutes
            retries: 5,
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
        let check = PingpongCheck;
        assert_eq!(check.name(), "pingpong");
        assert!(!check.description().is_empty());
    }

    #[test]
    fn test_default_options() {
        let check = PingpongCheck;
        let opts = check.default_options();
        assert_eq!(opts.retries, 5);
        assert_eq!(opts.timeout, Some(Duration::from_secs(300)));
    }
}
