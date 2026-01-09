//! Pushsync Check
//!
//! Verifies that chunks are correctly pushed to their closest node in Kademlia space.
//!
//! ## What it checks
//!
//! 1. Upload a random chunk to one node
//! 2. Calculate which node should store the chunk (closest overlay address)
//! 3. Verify the chunk exists on the closest node
//!
//! This tests the pushsync protocol - the mechanism by which chunks are automatically
//! routed to the node responsible for storing them based on Kademlia distance.
//!
//! ## Equivalent beekeeper check
//!
//! Reference: `beekeeper/pkg/check/pushsync/pushsync.go`
//!
//! ## Options
//!
//! - `chunks_per_node`: Number of chunks to upload per node (default: 1)
//! - `upload_node_count`: Number of nodes to upload from (default: 1)
//! - `seed`: Random seed for reproducible data (default: random)
//! - `retries`: Number of retries to check if chunk synced (default: 5)
//! - `retry_delay_ms`: Delay between retries in milliseconds (default: 1000)

use async_trait::async_trait;
use nectar_primitives::bytes::Bytes;
use nectar_primitives::{Chunk, ContentChunk, SwarmAddress};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

use super::traits::{Check, CheckContext, CheckError, CheckOptions, CheckResult, NodeResult};

/// Default chunks per upload node
const DEFAULT_CHUNKS_PER_NODE: usize = 1;

/// Default number of upload nodes
const DEFAULT_UPLOAD_NODE_COUNT: usize = 1;

/// Default retries for checking if chunk synced
const DEFAULT_RETRIES: usize = 5;

/// Default retry delay in milliseconds
const DEFAULT_RETRY_DELAY_MS: u64 = 1000;

/// Default retries for upload operations (like beekeeper's 3 upload retries)
const DEFAULT_UPLOAD_RETRIES: usize = 3;

/// Default upload retry wait in milliseconds (like beekeeper's TxOnErrWait = 10s)
const DEFAULT_UPLOAD_RETRY_WAIT_MS: u64 = 10_000;

/// Maximum chunk payload size (like beekeeper's MaxChunkSize)
const MAX_CHUNK_SIZE: usize = 4096;

/// Pushsync check for verifying Kademlia chunk routing
pub struct PushsyncCheck;

/// Simple xorshift PRNG for deterministic random generation
struct XorShiftRng {
    state: u64,
}

impl XorShiftRng {
    fn new(seed: u64) -> Self {
        // Ensure non-zero state
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn next(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    /// Generate a random value in range [0, max)
    fn next_usize(&mut self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }
        (self.next() as usize) % max
    }

    /// Fill a buffer with random bytes
    fn fill_bytes(&mut self, buf: &mut [u8]) {
        for byte in buf.iter_mut() {
            *byte = self.next() as u8;
        }
    }

    /// Create a child PRNG seeded from this one (like beekeeper's PseudoGenerators)
    fn child(&mut self) -> Self {
        Self::new(self.next())
    }
}

impl PushsyncCheck {
    /// Get overlay addresses for full nodes in the cluster (excludes bootnodes)
    ///
    /// Bootnodes are excluded because they have storageRadius=0 and don't
    /// participate in chunk storage. Only full nodes should be considered
    /// when determining the closest node for pushsync verification.
    async fn get_overlays(ctx: &CheckContext) -> Result<HashMap<String, SwarmAddress>, CheckError> {
        let mut overlays = HashMap::new();

        // Only include full nodes - bootnodes don't participate in chunk storage
        for node in ctx.full_nodes() {
            let node_name = node.name().unwrap_or("unknown").to_string();
            match node.addresses().await {
                Ok(addresses) => {
                    // Parse the overlay address into SwarmAddress
                    let overlay_hex = addresses.overlay.trim_start_matches("0x");
                    match hex::decode(overlay_hex) {
                        Ok(bytes) if bytes.len() == 32 => {
                            let mut arr = [0u8; 32];
                            arr.copy_from_slice(&bytes);
                            overlays.insert(node_name, SwarmAddress::new(arr));
                        }
                        _ => {
                            warn!(node = %node_name, overlay = %addresses.overlay, "Invalid overlay address format");
                        }
                    }
                }
                Err(e) => {
                    debug!(node = %node_name, error = %e, "Failed to get addresses");
                }
            }
        }

        if overlays.is_empty() {
            return Err(CheckError::Config(
                "Could not get overlay addresses from any node".to_string(),
            ));
        }

        Ok(overlays)
    }

    /// Find the closest node to a chunk address
    fn find_closest_node<'a>(
        chunk_address: &SwarmAddress,
        overlays: &'a HashMap<String, SwarmAddress>,
    ) -> Option<(&'a str, &'a SwarmAddress)> {
        overlays
            .iter()
            .min_by(|(_, a), (_, b)| {
                // Compare distances - closer means smaller XOR distance
                let dist_a = chunk_address.distance(a);
                let dist_b = chunk_address.distance(b);
                dist_a.cmp(&dist_b)
            })
            .map(|(name, addr)| (name.as_str(), addr))
    }

    /// Generate random chunk data with random size (like beekeeper's NewRandomChunk)
    /// Size is random 0-4095 bytes, matching beekeeper behavior: r.Intn(MaxChunkSize)
    fn generate_random_chunk(rng: &mut XorShiftRng) -> Vec<u8> {
        // Random size 0 to MAX_CHUNK_SIZE-1 (like Go's r.Intn(MaxChunkSize))
        let payload_size = rng.next_usize(MAX_CHUNK_SIZE);
        let mut payload = vec![0u8; payload_size];
        rng.fill_bytes(&mut payload);
        payload
    }
}

#[async_trait]
impl Check for PushsyncCheck {
    fn name(&self) -> &'static str {
        "pushsync"
    }

    fn description(&self) -> &'static str {
        "Verify chunks are pushed to closest node"
    }

    async fn run(
        &self,
        ctx: &CheckContext,
        opts: &CheckOptions,
    ) -> Result<CheckResult, CheckError> {
        let start = Instant::now();

        // Need at least 2 nodes
        if ctx.node_count() < 2 {
            return Err(CheckError::Config(
                "Pushsync check requires at least 2 nodes".to_string(),
            ));
        }

        // Get options
        let chunks_per_node: usize = opts
            .get_extra("chunks_per_node")
            .unwrap_or(DEFAULT_CHUNKS_PER_NODE);
        let upload_node_count: usize = opts
            .get_extra("upload_node_count")
            .unwrap_or(DEFAULT_UPLOAD_NODE_COUNT);
        let seed: u64 = opts.get_extra("seed").unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(42)
        });
        let retries: usize = opts.get_extra("retries").unwrap_or(DEFAULT_RETRIES);
        let retry_delay_ms: u64 = opts
            .get_extra("retry_delay_ms")
            .unwrap_or(DEFAULT_RETRY_DELAY_MS);

        info!(
            nodes = ctx.node_count(),
            chunks_per_node = chunks_per_node,
            upload_node_count = upload_node_count,
            seed = seed,
            "Starting pushsync check"
        );

        // Get overlay addresses for all nodes
        let overlays = Self::get_overlays(ctx).await?;
        info!(overlay_count = overlays.len(), "Got overlay addresses");

        // Batch options for creating batches (like beekeeper's GetOrCreateMutableBatch)
        let batch_opts = crate::client::BatchOptions::default();

        let mut all_results = Vec::new();
        let mut total_chunks = 0u64;
        let mut synced_chunks = 0u64;

        // Like beekeeper's checkChunks (chunks mode), use only full nodes for uploads
        // Bootnodes have fullNode=false for pushsync protocol and can't properly push chunks
        let full_nodes = ctx.full_nodes();
        if full_nodes.is_empty() {
            return Err(CheckError::Config(
                "Pushsync check requires at least one full node (not bootnode) for uploads"
                    .to_string(),
            ));
        }

        // Create per-node PRNGs like beekeeper's PseudoGenerators
        // Each upload node gets its own PRNG seeded from a master PRNG
        let mut master_rng = XorShiftRng::new(seed);
        let actual_upload_count = upload_node_count.min(full_nodes.len()).max(1);
        let mut node_rngs: Vec<XorShiftRng> = (0..actual_upload_count)
            .map(|_| master_rng.child())
            .collect();

        for upload_idx in 0..actual_upload_count {
            // Select upload node from full nodes only
            let upload_node = &full_nodes[upload_idx];
            let upload_node_name = upload_node.name().unwrap_or("unknown").to_string();

            // Get or create a batch on this upload node (like beekeeper's GetOrCreateMutableBatch)
            let node_batch_id = match upload_node.get_or_create_batch(&batch_opts).await {
                Ok(batch_id) => {
                    debug!(node = %upload_node_name, batch_id = %batch_id, "Got batch for uploads");
                    batch_id
                }
                Err(e) => {
                    error!(node = %upload_node_name, error = %e, "Failed to get/create batch");
                    all_results.push(NodeResult::failed(
                        &upload_node_name,
                        format!("Failed to get/create batch: {}", e),
                    ));
                    continue;
                }
            };

            // Get this node's PRNG
            let rng = &mut node_rngs[upload_idx];

            for chunk_idx in 0..chunks_per_node {
                total_chunks += 1;

                // Generate random chunk data with random size (0-4095 bytes)
                let payload = Self::generate_random_chunk(rng);
                let payload_bytes = Bytes::from(payload.clone());

                // Create ContentChunk to get the correct BMT address
                let chunk = match ContentChunk::new(payload_bytes) {
                    Ok(c) => c,
                    Err(e) => {
                        error!(error = %e, "Failed to create chunk");
                        all_results.push(NodeResult::failed(
                            &upload_node_name,
                            format!("Failed to create chunk: {}", e),
                        ));
                        continue;
                    }
                };

                let chunk_address = *chunk.address();
                let chunk_address_hex = hex::encode(chunk_address.as_bytes());

                debug!(
                    node = %upload_node_name,
                    chunk = %chunk_address_hex,
                    chunk_idx = chunk_idx,
                    "Uploading chunk"
                );

                // Upload chunk (need to include span prefix for chunks endpoint)
                let span = (payload.len() as u64).to_le_bytes();
                let mut chunk_data = Vec::with_capacity(8 + payload.len());
                chunk_data.extend_from_slice(&span);
                chunk_data.extend_from_slice(&payload);

                // Upload with retries (like beekeeper's upload retry loop)
                let mut reference: Option<String> = None;
                let mut last_upload_error: Option<String> = None;

                for attempt in 0..DEFAULT_UPLOAD_RETRIES {
                    if attempt > 0 {
                        info!(
                            node = %upload_node_name,
                            attempt = attempt + 1,
                            retries = DEFAULT_UPLOAD_RETRIES,
                            wait_ms = DEFAULT_UPLOAD_RETRY_WAIT_MS,
                            "Retrying upload"
                        );
                        tokio::time::sleep(Duration::from_millis(DEFAULT_UPLOAD_RETRY_WAIT_MS))
                            .await;
                    }

                    match upload_node
                        .upload_chunk(&node_batch_id, chunk_data.clone())
                        .await
                    {
                        Ok(resp) => {
                            reference = Some(resp.reference);
                            break;
                        }
                        Err(e) => {
                            warn!(node = %upload_node_name, error = %e, attempt = attempt + 1, "Upload attempt failed");
                            last_upload_error = Some(format!("{e}"));
                        }
                    }
                }

                match reference {
                    Some(ref uploaded_reference) => {
                        info!(
                            node = %upload_node_name,
                            reference = %uploaded_reference,
                            expected = %chunk_address_hex,
                            "Chunk uploaded"
                        );

                        // Find the closest node
                        let (closest_name, closest_overlay) =
                            match Self::find_closest_node(&chunk_address, &overlays) {
                                Some(result) => result,
                                None => {
                                    error!("No closest node found");
                                    all_results.push(NodeResult::failed(
                                        &upload_node_name,
                                        "No closest node found",
                                    ));
                                    continue;
                                }
                            };

                        let proximity = chunk_address.proximity(closest_overlay);
                        info!(
                            closest_node = %closest_name,
                            proximity = proximity,
                            "Closest node determined"
                        );

                        // Find the closest node's client
                        let closest_node = ctx
                            .nodes
                            .iter()
                            .find(|n| n.name().map(|name| name == closest_name).unwrap_or(false));

                        let closest_node = match closest_node {
                            Some(n) => n,
                            None => {
                                error!(node = %closest_name, "Closest node not found in context");
                                all_results.push(NodeResult::failed(
                                    closest_name,
                                    "Node not found in context",
                                ));
                                continue;
                            }
                        };

                        // Check if chunk exists on closest node with retries
                        let mut synced = false;
                        for retry in 0..=retries {
                            if retry > 0 {
                                tokio::time::sleep(Duration::from_millis(retry_delay_ms)).await;
                            }

                            match closest_node.has_chunk(uploaded_reference).await {
                                Ok(true) => {
                                    synced = true;
                                    break;
                                }
                                Ok(false) => {
                                    debug!(
                                        node = %closest_name,
                                        chunk = %uploaded_reference,
                                        retry = retry,
                                        "Chunk not found, retrying..."
                                    );
                                }
                                Err(e) => {
                                    warn!(
                                        node = %closest_name,
                                        error = %e,
                                        retry = retry,
                                        "Error checking chunk"
                                    );
                                }
                            }
                        }

                        if synced {
                            synced_chunks += 1;
                            info!(
                                node = %closest_name,
                                chunk = %uploaded_reference,
                                "Chunk synced to closest node"
                            );
                            all_results.push(
                                NodeResult::passed(closest_name)
                                    .with_detail("action", "pushsync")
                                    .with_detail("chunk", uploaded_reference.clone())
                                    .with_detail("proximity", proximity as usize),
                            );
                        } else {
                            error!(
                                node = %closest_name,
                                chunk = %uploaded_reference,
                                "Chunk NOT found on closest node after retries"
                            );
                            all_results.push(
                                NodeResult::failed(
                                    closest_name,
                                    format!(
                                        "Chunk {} not synced after {} retries",
                                        uploaded_reference, retries
                                    ),
                                )
                                .with_detail("action", "pushsync")
                                .with_detail("chunk", uploaded_reference),
                            );
                        }
                    }
                    None => {
                        let err_msg =
                            last_upload_error.unwrap_or_else(|| "Unknown error".to_string());
                        error!(node = %upload_node_name, error = %err_msg, retries = DEFAULT_UPLOAD_RETRIES, "Upload failed after all retries");
                        all_results.push(NodeResult::failed(
                            &upload_node_name,
                            format!(
                                "Upload failed after {} attempts: {}",
                                DEFAULT_UPLOAD_RETRIES, err_msg
                            ),
                        ));
                    }
                }
            }
        }

        let duration = start.elapsed();
        let passed = synced_chunks == total_chunks && total_chunks > 0;

        info!(
            passed = passed,
            synced = synced_chunks,
            total = total_chunks,
            duration_secs = duration.as_secs(),
            "Pushsync check complete"
        );

        let message = format!(
            "{}/{} chunks synced to closest nodes",
            synced_chunks, total_chunks
        );

        Ok(CheckResult::new("pushsync", all_results, duration).with_message(message))
    }

    fn default_options(&self) -> CheckOptions {
        CheckOptions {
            timeout: Some(Duration::from_secs(300)),
            retries: 3,
            retry_delay: Some(Duration::from_secs(5)),
            extra: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_metadata() {
        let check = PushsyncCheck;
        assert_eq!(check.name(), "pushsync");
        assert!(!check.description().is_empty());
    }

    #[test]
    fn test_default_options() {
        let check = PushsyncCheck;
        let opts = check.default_options();
        assert_eq!(opts.timeout, Some(Duration::from_secs(300)));
    }

    #[test]
    fn test_generate_random_chunk_deterministic() {
        let mut rng1 = super::XorShiftRng::new(12345);
        let mut rng2 = super::XorShiftRng::new(12345);
        let data1 = PushsyncCheck::generate_random_chunk(&mut rng1);
        let data2 = PushsyncCheck::generate_random_chunk(&mut rng2);
        assert_eq!(data1, data2);
    }

    #[test]
    fn test_generate_random_chunk_variable_size() {
        let mut rng = super::XorShiftRng::new(12345);
        // Generate multiple chunks, they should have varying sizes
        let sizes: Vec<usize> = (0..10)
            .map(|_| PushsyncCheck::generate_random_chunk(&mut rng).len())
            .collect();
        // With high probability, not all sizes should be the same
        let first = sizes[0];
        let all_same = sizes.iter().all(|&s| s == first);
        // This is probabilistic but very unlikely to fail
        assert!(!all_same || sizes.len() < 2, "Chunk sizes should vary");
    }

    #[test]
    fn test_per_node_prng_independence() {
        // Like beekeeper's PseudoGenerators - each node gets independent PRNG
        let mut master1 = super::XorShiftRng::new(12345);
        let mut master2 = super::XorShiftRng::new(12345);

        let mut child1_a = master1.child();
        let mut child1_b = master1.child();

        let mut child2_a = master2.child();
        let mut child2_b = master2.child();

        // Same master seed should produce same child sequences
        assert_eq!(child1_a.next(), child2_a.next());
        assert_eq!(child1_b.next(), child2_b.next());

        // Different children from same master should be independent
        let mut fresh_master = super::XorShiftRng::new(12345);
        let mut c1 = fresh_master.child();
        let mut c2 = fresh_master.child();
        assert_ne!(c1.next(), c2.next());
    }

    #[test]
    fn test_find_closest_node() {
        let mut overlays = HashMap::new();

        // Create test addresses
        let target = SwarmAddress::new([0x10; 32]); // 0x1010...
        let close = SwarmAddress::new([0x11; 32]); // 0x1111... (close to target)
        let far = SwarmAddress::new([0xF0; 32]); // 0xF0F0... (far from target)

        overlays.insert("close-node".to_string(), close);
        overlays.insert("far-node".to_string(), far);

        let (name, _) = PushsyncCheck::find_closest_node(&target, &overlays).unwrap();
        assert_eq!(name, "close-node");
    }
}
