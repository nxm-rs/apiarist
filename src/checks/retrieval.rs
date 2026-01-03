//! Retrieval Check
//!
//! Verifies that chunks can be retrieved from different nodes in the cluster.
//!
//! ## What it checks
//!
//! 1. Upload a random chunk to one node
//! 2. Download the chunk from a different node
//! 3. Verify the downloaded data matches the original
//!
//! This tests the retrieval protocol - the mechanism by which nodes can
//! request and receive chunks from other nodes in the network.
//!
//! ## Equivalent beekeeper check
//!
//! Reference: `beekeeper/pkg/check/retrieval/retrieval.go`
//!
//! ## Options
//!
//! - `chunks_per_node`: Number of chunks to upload per node (default: 1)
//! - `upload_node_count`: Number of nodes to upload from (default: 1)
//! - `seed`: Random seed for reproducible data (default: random)

use async_trait::async_trait;
use nectar_primitives::bytes::Bytes;
use nectar_primitives::{Chunk, ContentChunk};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

use super::traits::{Check, CheckContext, CheckError, CheckOptions, CheckResult, NodeResult};

/// Default chunks per upload node
const DEFAULT_CHUNKS_PER_NODE: usize = 1;

/// Default number of upload nodes
const DEFAULT_UPLOAD_NODE_COUNT: usize = 1;

/// Maximum chunk payload size (like beekeeper's MaxChunkSize)
const MAX_CHUNK_SIZE: usize = 4096;

/// Retrieval check for verifying chunk retrieval across nodes
pub struct RetrievalCheck;

/// Simple xorshift PRNG for deterministic random generation
struct XorShiftRng {
    state: u64,
}

impl XorShiftRng {
    fn new(seed: u64) -> Self {
        // Ensure non-zero state
        Self { state: if seed == 0 { 1 } else { seed } }
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

impl RetrievalCheck {
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
impl Check for RetrievalCheck {
    fn name(&self) -> &'static str {
        "retrieval"
    }

    fn description(&self) -> &'static str {
        "Verify chunks can be retrieved from different nodes"
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
                "Retrieval check requires at least 2 nodes".to_string(),
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

        info!(
            nodes = ctx.node_count(),
            chunks_per_node = chunks_per_node,
            upload_node_count = upload_node_count,
            seed = seed,
            "Starting retrieval check"
        );

        // Batch options for creating batches (like beekeeper's GetOrCreateMutableBatch)
        let batch_opts = crate::client::BatchOptions::default();

        let mut all_results = Vec::new();
        let mut total_chunks = 0u64;
        let mut retrieved_chunks = 0u64;
        let mut mismatches = 0u64;

        // Get full nodes for uploads (beekeeper uses FullNodeNames)
        // Note: boot nodes can't pushsync chunks properly, so use only true full nodes
        let full_nodes = ctx.full_nodes();
        if full_nodes.is_empty() {
            return Err(CheckError::Config(
                "Retrieval check requires at least one full node (not bootnode) for uploads".to_string(),
            ));
        }
        let actual_upload_count = upload_node_count.min(full_nodes.len()).max(1);

        // Create per-node PRNGs like beekeeper's PseudoGenerators
        // Each upload node gets its own PRNG seeded from a master PRNG
        let mut master_rng = XorShiftRng::new(seed);
        let mut node_rngs: Vec<XorShiftRng> = (0..actual_upload_count)
            .map(|_| master_rng.child())
            .collect();

        for upload_idx in 0..actual_upload_count {
            // Select upload node - cycle through full nodes like beekeeper does
            let upload_node = &full_nodes[upload_idx];
            let upload_node_name = upload_node.name().unwrap_or("unknown").to_string();

            // Calculate download node (round-robin: next node in full nodes list)
            let download_idx = (upload_idx + 1) % full_nodes.len();
            let download_node = &full_nodes[download_idx];
            let download_node_name = download_node.name().unwrap_or("unknown").to_string();

            // Get or create a batch on this upload node (like beekeeper's GetOrCreateMutableBatch)
            let node_batch_id = match upload_node.get_or_create_batch(&batch_opts).await {
                Ok(batch_id) => {
                    debug!(node = %upload_node_name, batch_id = %batch_id, "Got batch for uploads");
                    batch_id
                }
                Err(e) => {
                    error!(node = %upload_node_name, error = %e, "Failed to get/create batch");
                    all_results.push(
                        NodeResult::failed(&upload_node_name, format!("Failed to get/create batch: {}", e))
                    );
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

                // Create ContentChunk to calculate address
                let chunk = match ContentChunk::new(payload_bytes) {
                    Ok(c) => c,
                    Err(e) => {
                        error!(error = %e, "Failed to create chunk");
                        all_results.push(
                            NodeResult::failed(&upload_node_name, format!("Failed to create chunk: {}", e))
                        );
                        continue;
                    }
                };

                let chunk_address = chunk.address();
                let chunk_address_hex = hex::encode(chunk_address.as_bytes());

                debug!(
                    upload_node = %upload_node_name,
                    download_node = %download_node_name,
                    chunk = %chunk_address_hex,
                    chunk_idx = chunk_idx,
                    "Testing retrieval"
                );

                // Upload chunk with span prefix
                let span = (payload.len() as u64).to_le_bytes();
                let mut chunk_data = Vec::with_capacity(8 + payload.len());
                chunk_data.extend_from_slice(&span);
                chunk_data.extend_from_slice(&payload);

                let upload_result = upload_node.upload_chunk(&node_batch_id, chunk_data).await;

                let reference = match upload_result {
                    Ok(resp) => {
                        info!(
                            node = %upload_node_name,
                            reference = %resp.reference,
                            "Chunk uploaded"
                        );
                        all_results.push(
                            NodeResult::passed(&upload_node_name)
                                .with_detail("action", "upload")
                                .with_detail("reference", resp.reference.clone())
                        );
                        resp.reference
                    }
                    Err(e) => {
                        error!(node = %upload_node_name, error = %e, "Upload failed");
                        all_results.push(
                            NodeResult::failed(&upload_node_name, format!("Upload failed: {}", e))
                        );
                        continue;
                    }
                };

                // Small delay to allow propagation
                tokio::time::sleep(Duration::from_millis(100)).await;

                // Download chunk from different node
                let download_result = download_node.download_chunk(&reference).await;

                match download_result {
                    Ok(downloaded_data) => {
                        // Compare full chunk data (span + payload) like beekeeper does
                        // chunk_data contains: span (8 bytes) + payload
                        let span = (payload.len() as u64).to_le_bytes();
                        let mut expected_data = Vec::with_capacity(8 + payload.len());
                        expected_data.extend_from_slice(&span);
                        expected_data.extend_from_slice(&payload);

                        if downloaded_data == expected_data {
                            retrieved_chunks += 1;
                            info!(
                                node = %download_node_name,
                                reference = %reference,
                                "Chunk retrieved successfully, data matches"
                            );
                            all_results.push(
                                NodeResult::passed(&download_node_name)
                                    .with_detail("action", "download")
                                    .with_detail("reference", reference)
                            );
                        } else {
                            mismatches += 1;
                            warn!(
                                node = %download_node_name,
                                reference = %reference,
                                expected_size = expected_data.len(),
                                actual_size = downloaded_data.len(),
                                "Data mismatch"
                            );
                            all_results.push(
                                NodeResult::failed(
                                    &download_node_name,
                                    format!("Data mismatch: expected {} bytes, got {}", expected_data.len(), downloaded_data.len())
                                )
                                .with_detail("action", "download")
                                .with_detail("reference", reference)
                            );
                        }
                    }
                    Err(e) => {
                        warn!(
                            node = %download_node_name,
                            reference = %reference,
                            error = %e,
                            "Download failed"
                        );
                        all_results.push(
                            NodeResult::failed(&download_node_name, format!("Download failed: {}", e))
                                .with_detail("action", "download")
                                .with_detail("reference", reference)
                        );
                    }
                }
            }
        }

        let duration = start.elapsed();
        let passed = retrieved_chunks == total_chunks && total_chunks > 0 && mismatches == 0;

        info!(
            passed = passed,
            retrieved = retrieved_chunks,
            total = total_chunks,
            mismatches = mismatches,
            duration_secs = duration.as_secs(),
            "Retrieval check complete"
        );

        let message = format!(
            "{}/{} chunks retrieved successfully, {} mismatches",
            retrieved_chunks, total_chunks, mismatches
        );

        Ok(CheckResult::new("retrieval", all_results, duration).with_message(message))
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
        let check = RetrievalCheck;
        assert_eq!(check.name(), "retrieval");
        assert!(!check.description().is_empty());
    }

    #[test]
    fn test_default_options() {
        let check = RetrievalCheck;
        let opts = check.default_options();
        assert_eq!(opts.timeout, Some(Duration::from_secs(300)));
    }

    #[test]
    fn test_generate_random_chunk_deterministic() {
        let mut rng1 = super::XorShiftRng::new(12345);
        let mut rng2 = super::XorShiftRng::new(12345);
        let data1 = RetrievalCheck::generate_random_chunk(&mut rng1);
        let data2 = RetrievalCheck::generate_random_chunk(&mut rng2);
        assert_eq!(data1, data2);
    }

    #[test]
    fn test_generate_random_chunk_different_seeds() {
        let mut rng1 = super::XorShiftRng::new(12345);
        let mut rng2 = super::XorShiftRng::new(54321);
        let data1 = RetrievalCheck::generate_random_chunk(&mut rng1);
        let data2 = RetrievalCheck::generate_random_chunk(&mut rng2);
        // Different seeds should produce different data (sizes may also differ)
        assert_ne!(data1, data2);
    }

    #[test]
    fn test_generate_random_chunk_variable_size() {
        let mut rng = super::XorShiftRng::new(12345);
        // Generate multiple chunks, they should have varying sizes
        let sizes: Vec<usize> = (0..10)
            .map(|_| RetrievalCheck::generate_random_chunk(&mut rng).len())
            .collect();
        // With high probability, not all sizes should be the same
        let first = sizes[0];
        let all_same = sizes.iter().all(|&s| s == first);
        // This is probabilistic but very unlikely to fail
        assert!(!all_same || sizes.len() < 2, "Chunk sizes should vary");
    }
}
