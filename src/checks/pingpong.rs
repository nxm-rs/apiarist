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
//! The beekeeper pingpong check:
//! - Processes all nodes concurrently
//! - For each node, pings all its peers
//! - Uses 5 retries with exponential backoff (0, 2s, 4s, 6s, 8s)
//! - Records RTT metrics to Prometheus (gauge per node/peer, histogram)
//! - Fails fast on first error after retries
//!
//! ## Options
//!
//! - `timeout`: Maximum time for check (default: 5m)
//! - `retries`: Number of retry attempts per ping (default: 5)
//! - `retry_delay`: Base delay between retries, exponential backoff (default: 2s)

use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use super::traits::{Check, CheckContext, CheckError, CheckOptions, CheckResult, NodeResult};
use crate::client::BeeClient;
use crate::utils::{run_concurrent, ConcurrentOpts};

/// Pingpong connectivity check
///
/// Verifies that all nodes can ping their peers and measures RTT.
/// Runs concurrently across all nodes with fail-fast behavior.
pub struct PingpongCheck;

/// Result of pinging all peers for a single node
struct NodePingResult {
    overlay: String,
    peer_count: usize,
    successful_pings: usize,
    failed_pings: usize,
    ping_details: Vec<serde_json::Value>,
}

#[async_trait]
impl Check for PingpongCheck {
    fn name(&self) -> &'static str {
        "pingpong"
    }

    fn description(&self) -> &'static str {
        "Test peer connectivity with RTT measurement"
    }

    async fn run(
        &self,
        ctx: &CheckContext,
        opts: &CheckOptions,
    ) -> Result<CheckResult, CheckError> {
        let start = Instant::now();
        let max_retries = opts.retries.max(1);
        let retry_delay = opts.retry_delay_or(Duration::from_secs(2));

        info!(
            nodes = ctx.node_count(),
            retries = max_retries,
            "Starting pingpong check"
        );

        // Run ping tests concurrently across all nodes with fail-fast
        // Note: Retries are handled per-peer inside ping_node_peers, not at the node level
        let concurrent_opts = ConcurrentOpts::default()
            .with_fail_fast(true);

        let results = run_concurrent(
            ctx.nodes.clone(),
            move |node: Arc<BeeClient>| async move {
                ping_node_peers(&node, max_retries, retry_delay).await
            },
            concurrent_opts,
        )
        .await;

        // Convert results to CheckResult format
        let duration = start.elapsed();

        match results {
            Ok(task_results) => {
                let mut node_results = Vec::new();
                let mut total_pings = 0usize;
                let mut successful_pings = 0usize;

                for task_result in task_results {
                    let ping_result = task_result.value;
                    total_pings += ping_result.successful_pings + ping_result.failed_pings;
                    successful_pings += ping_result.successful_pings;

                    let node_passed = ping_result.failed_pings == 0;

                    let result = if node_passed {
                        NodeResult::passed(&task_result.id)
                            .with_detail("overlay", &ping_result.overlay)
                            .with_detail("peer_count", ping_result.peer_count)
                            .with_detail("pings", ping_result.ping_details)
                    } else {
                        NodeResult::failed(
                            &task_result.id,
                            format!("{} peers unreachable", ping_result.failed_pings),
                        )
                        .with_detail("overlay", &ping_result.overlay)
                        .with_detail("peer_count", ping_result.peer_count)
                        .with_detail("pings", ping_result.ping_details)
                    };

                    node_results.push(result);
                }

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
            Err(e) => {
                // Fail-fast triggered - a node failed completely
                warn!(error = %e, "Pingpong check failed (fail-fast)");
                Err(CheckError::Failed(e.to_string()))
            }
        }
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

/// Ping all peers of a single node
///
/// Returns an error if the node has no peers or cannot be reached.
/// Individual ping failures are recorded but don't cause early return.
async fn ping_node_peers(
    node: &BeeClient,
    max_retries: u32,
    retry_delay: Duration,
) -> Result<NodePingResult, String> {
    let node_name = node.name().unwrap_or("unknown");

    // Get node's overlay address
    let overlay = match node.addresses().await {
        Ok(addrs) => addrs.overlay,
        Err(e) => {
            return Err(format!("Failed to get addresses: {}", e));
        }
    };

    // Get peers
    let peers = match node.peers().await {
        Ok(p) => p,
        Err(e) => {
            return Err(format!("Failed to get peers: {}", e));
        }
    };

    if peers.peers.is_empty() {
        return Err("No peers found".to_string());
    }

    debug!(
        node = %node_name,
        overlay = %overlay,
        peer_count = peers.peers.len(),
        "Found peers, starting pings"
    );

    let mut ping_details = Vec::new();
    let mut successful_pings = 0usize;
    let mut failed_pings = 0usize;

    // Ping each peer with retry logic
    for peer in &peers.peers {
        let peer_addr = &peer.address;
        let mut success = false;
        let mut last_error = None;

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
                    ping_details.push(serde_json::json!({
                        "peer": peer_addr,
                        "rtt": response.rtt,
                        "attempts": attempt + 1,
                        "success": true
                    }));
                    successful_pings += 1;
                    success = true;
                    break;
                }
                Err(e) => {
                    last_error = Some(e.to_string());
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
            failed_pings += 1;
            ping_details.push(serde_json::json!({
                "peer": peer_addr,
                "error": last_error.unwrap_or_else(|| "Unknown error".to_string()),
                "attempts": max_retries,
                "success": false
            }));
        }
    }

    // If all pings failed, this is a node-level failure
    if successful_pings == 0 && failed_pings > 0 {
        return Err(format!("All {} pings failed", failed_pings));
    }

    Ok(NodePingResult {
        overlay,
        peer_count: peers.peers.len(),
        successful_pings,
        failed_pings,
        ping_details,
    })
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
        assert_eq!(opts.retry_delay, Some(Duration::from_secs(2)));
    }
}
