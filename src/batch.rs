//! Batch preparation module
//!
//! Pre-creates postage batches on all full nodes before running checks.
//! This avoids the 10-block wait time per check by creating all batches
//! upfront and waiting once for them all to become usable.

use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::checks::CheckContext;
use crate::client::{BatchOptions, BeeClient};

/// Default batch options for check preparation
const DEFAULT_BATCH_DEPTH: u8 = 17;
const DEFAULT_BATCH_LABEL: &str = "apiarist";
const DEFAULT_BATCH_TTL_SECS: u64 = 24 * 60 * 60; // 24 hours

/// Result of batch preparation
#[derive(Debug)]
pub struct BatchPreparationResult {
    /// Number of batches created
    pub batches_created: usize,
    /// Number of nodes that already had usable batches
    pub batches_reused: usize,
    /// Total time spent preparing batches
    pub duration: Duration,
}

/// Prepare postage batches on all full nodes
///
/// This creates batches on all full-capable nodes (excluding bootnodes which
/// don't store data) and waits for them to become usable before returning.
///
/// # Arguments
/// * `ctx` - Check context with node information
/// * `opts` - Optional batch options (uses defaults if None)
///
/// # Returns
/// * `Ok(BatchPreparationResult)` - Summary of batch preparation
/// * `Err` - If batch creation fails on all nodes
pub async fn prepare_batches(
    ctx: &CheckContext,
    opts: Option<BatchOptions>,
) -> anyhow::Result<BatchPreparationResult> {
    let start = std::time::Instant::now();
    let batch_opts = opts.unwrap_or_else(|| BatchOptions {
        depth: DEFAULT_BATCH_DEPTH,
        label: Some(DEFAULT_BATCH_LABEL.to_string()),
        ttl_secs: DEFAULT_BATCH_TTL_SECS,
        fallback_amount: "100000000".to_string(),
    });

    let full_nodes = ctx.full_nodes();
    info!(
        node_count = full_nodes.len(),
        label = ?batch_opts.label,
        depth = batch_opts.depth,
        "Preparing postage batches on full nodes"
    );

    if full_nodes.is_empty() {
        warn!("No full nodes found for batch preparation");
        return Ok(BatchPreparationResult {
            batches_created: 0,
            batches_reused: 0,
            duration: start.elapsed(),
        });
    }

    // Create batches on all full nodes concurrently
    let mut handles = Vec::new();
    for node in full_nodes {
        let node = node.clone();
        let opts = batch_opts.clone();
        handles.push(tokio::spawn(async move {
            create_batch_on_node(&node, &opts).await
        }));
    }

    // Wait for all batch creations to complete
    let mut batches_created = 0;
    let mut batches_reused = 0;

    for handle in handles {
        match handle.await {
            Ok(Ok((batch_id, was_created))) => {
                if was_created {
                    batches_created += 1;
                } else {
                    batches_reused += 1;
                }
                debug!(batch_id = %batch_id, created = was_created, "Batch ready");
            }
            Ok(Err(e)) => {
                warn!(error = %e, "Failed to prepare batch on node");
            }
            Err(e) => {
                warn!(error = %e, "Batch preparation task panicked");
            }
        }
    }

    let duration = start.elapsed();
    info!(
        created = batches_created,
        reused = batches_reused,
        duration_secs = duration.as_secs(),
        "Batch preparation complete"
    );

    if batches_created == 0 && batches_reused == 0 {
        anyhow::bail!("Failed to prepare any batches");
    }

    Ok(BatchPreparationResult {
        batches_created,
        batches_reused,
        duration,
    })
}

/// Create or find a batch on a single node
///
/// Returns (batch_id, was_created) where was_created is true if a new batch
/// was created, false if an existing batch was reused.
async fn create_batch_on_node(
    node: &Arc<BeeClient>,
    opts: &BatchOptions,
) -> anyhow::Result<(String, bool)> {
    let node_name = node.name().unwrap_or("unknown");
    debug!(node = %node_name, "Checking for existing batch");

    // First check if there's already a usable batch
    let batches = node.list_stamps().await?;
    for batch in batches.stamps {
        if !batch.usable {
            continue;
        }
        if batch.immutable {
            continue;
        }
        if let Some(ref label) = opts.label {
            match &batch.label {
                Some(batch_label) if batch_label == label => {}
                _ => continue,
            }
        }
        // Check TTL
        if let Some(ttl) = batch.batch_ttl {
            if ttl == 0 {
                continue;
            }
        }
        // Check capacity
        let capacity = 1u32 << (batch.depth.saturating_sub(batch.bucket_depth));
        if batch.utilization >= capacity {
            continue;
        }

        debug!(
            node = %node_name,
            batch_id = %batch.batch_id,
            "Found existing usable batch"
        );
        return Ok((batch.batch_id, false));
    }

    // No existing batch found, create a new one
    info!(node = %node_name, "Creating new batch");
    let batch_id = node.get_or_create_batch(opts).await?;
    Ok((batch_id, true))
}
