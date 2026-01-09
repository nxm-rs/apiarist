//! Pullsync Check
//!
//! Verifies that chunks are correctly pulled/replicated to all nodes within their
//! Kademlia neighborhood responsibility.
//!
//! ## What it tests
//!
//! This check tests the **pull-sync wire protocol** - the mechanism by which nodes
//! actively pull chunks from their neighbors that fall within their storage responsibility.
//!
//! 1. Upload a random chunk to one node
//! 2. Find the closest node to the chunk address
//! 3. Get the closest node's topology (connected peers)
//! 4. For each connected peer, check if `proximity(chunk, peer) >= peer.depth`
//! 5. Verify the chunk exists on ALL such nodes (neighborhood replication)
//! 6. Calculate global replication factor and verify it meets the threshold
//!
//! ## Difference from pushsync
//!
//! - **Pushsync**: Tests that chunks are PUSHED to the closest node during upload
//! - **Pullsync**: Tests that chunks are PULLED by all nodes in the neighborhood
//!
//! Pullsync is critical for network consistency - if a node misses a push, pullsync
//! ensures it eventually gets the chunk by pulling from neighbors.
//!
//! ## Equivalent beekeeper check
//!
//! Reference: `beekeeper/pkg/check/pullsync/pullsync.go`
//!
//! ## Options
//!
//! - `chunks_per_node`: Number of chunks to upload per node (default: 1)
//! - `upload_node_count`: Number of nodes to upload from (default: 1)
//! - `seed`: Random seed for reproducible data (default: random)
//! - `replication_factor_threshold`: Minimum required replication factor (default: 2)

use async_trait::async_trait;
use nectar_primitives::bytes::Bytes;
use nectar_primitives::{Chunk, ContentChunk, SwarmAddress};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use super::traits::{Check, CheckContext, CheckError, CheckOptions, CheckResult, NodeResult};

/// Default chunks per upload node
const DEFAULT_CHUNKS_PER_NODE: usize = 1;

/// Default number of upload nodes
const DEFAULT_UPLOAD_NODE_COUNT: usize = 1;

/// Default minimum replication factor
const DEFAULT_REPLICATION_FACTOR_THRESHOLD: usize = 2;

/// Number of retry attempts (like beekeeper's t < 5, so 4 attempts)
const RETRY_ATTEMPTS: usize = 4;

/// Maximum chunk payload size (like beekeeper's MaxChunkSize)
const MAX_CHUNK_SIZE: usize = 4096;

/// Pullsync check for verifying Kademlia neighborhood replication
pub struct PullsyncCheck;

/// Node topology information needed for replication calculations
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used in HashMap lookups
struct NodeTopology {
    /// Node name
    name: String,
    /// Overlay address
    overlay: SwarmAddress,
    /// Kademlia depth (storage radius)
    depth: u8,
}

/// Simple xorshift PRNG for deterministic random generation
struct XorShiftRng {
    state: u64,
}

impl XorShiftRng {
    fn new(seed: u64) -> Self {
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

    /// Create a child PRNG seeded from this one
    fn child(&mut self) -> Self {
        Self::new(self.next())
    }
}

impl PullsyncCheck {
    /// Get overlay addresses for all nodes in the cluster
    ///
    /// Returns a map of node name -> overlay address.
    async fn get_overlays(ctx: &CheckContext) -> Result<HashMap<String, SwarmAddress>, CheckError> {
        let mut overlays = HashMap::new();

        for node in &ctx.nodes {
            let node_name = node.name().unwrap_or("unknown").to_string();
            match node.addresses().await {
                Ok(addresses) => {
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

    /// Get topologies (depth) for all nodes in the cluster
    ///
    /// Returns a map of node name -> NodeTopology.
    async fn get_topologies(
        ctx: &CheckContext,
        overlays: &HashMap<String, SwarmAddress>,
    ) -> Result<HashMap<String, NodeTopology>, CheckError> {
        let mut topologies = HashMap::new();

        for node in &ctx.nodes {
            let node_name = node.name().unwrap_or("unknown").to_string();

            // Get overlay from pre-fetched map
            let overlay = match overlays.get(&node_name) {
                Some(o) => *o,
                None => continue,
            };

            // Get topology for depth
            let depth = match node.topology().await {
                Ok(topology) => topology.depth as u8,
                Err(e) => {
                    warn!(node = %node_name, error = %e, "Failed to get topology, assuming depth=0");
                    0
                }
            };

            topologies.insert(
                node_name.clone(),
                NodeTopology {
                    name: node_name,
                    overlay,
                    depth,
                },
            );
        }

        if topologies.is_empty() {
            return Err(CheckError::Config(
                "Could not get topology from any node".to_string(),
            ));
        }

        Ok(topologies)
    }

    /// Find the closest node to a chunk address
    fn find_closest_node<'a>(
        chunk_address: &SwarmAddress,
        overlays: &'a HashMap<String, SwarmAddress>,
    ) -> Option<(&'a str, &'a SwarmAddress)> {
        overlays
            .iter()
            .min_by(|(_, a), (_, b)| {
                let dist_a = chunk_address.distance(a);
                let dist_b = chunk_address.distance(b);
                dist_a.cmp(&dist_b)
            })
            .map(|(name, addr)| (name.as_str(), addr))
    }

    /// Find replicating nodes by checking connected peers of the closest node.
    ///
    /// This matches beekeeper's algorithm:
    /// 1. Find the closest node
    /// 2. Get the closest node's topology (connected peers)
    /// 3. For each connected peer, check if proximity(chunk, peer) >= peer.depth
    ///
    /// Returns the names of nodes that should replicate the chunk.
    async fn find_replicating_nodes(
        ctx: &CheckContext,
        chunk_address: &SwarmAddress,
        overlays: &HashMap<String, SwarmAddress>,
        topologies: &HashMap<String, NodeTopology>,
    ) -> Result<Vec<String>, CheckError> {
        // Find the closest node
        let (closest_name, _closest_overlay) = Self::find_closest_node(chunk_address, overlays)
            .ok_or_else(|| CheckError::Config("No nodes available".to_string()))?;

        info!(
            closest_node = %closest_name,
            chunk = %hex::encode(&chunk_address.as_bytes()[..8]),
            "Found closest node for chunk"
        );

        // Find the closest node's client
        let closest_client = ctx
            .nodes
            .iter()
            .find(|n| n.name().map(|name| name == closest_name).unwrap_or(false))
            .ok_or_else(|| {
                CheckError::Config(format!(
                    "Closest node {} not found in context",
                    closest_name
                ))
            })?;

        // Get fresh topology from the closest node
        let closest_topology = closest_client.topology().await.map_err(|e| {
            CheckError::Failed(format!(
                "Failed to get topology from closest node {}: {}",
                closest_name, e
            ))
        })?;

        // Iterate through closest node's bins to find connected peers
        let mut replicating_nodes: HashMap<String, SwarmAddress> = HashMap::new();
        let mut nn_rep = 0;

        for bin in closest_topology.bins.values() {
            if let Some(connected_peers) = &bin.connected_peers {
                for peer in connected_peers {
                    // Parse peer address
                    let peer_overlay_hex = peer.address.trim_start_matches("0x");
                    let peer_overlay = match hex::decode(peer_overlay_hex) {
                        Ok(bytes) if bytes.len() == 32 => {
                            let mut arr = [0u8; 32];
                            arr.copy_from_slice(&bytes);
                            SwarmAddress::new(arr)
                        }
                        _ => {
                            warn!(peer = %peer.address, "Invalid peer overlay format");
                            continue;
                        }
                    };

                    // Find this peer's name from overlays
                    let peer_name = overlays
                        .iter()
                        .find(|(_, addr)| **addr == peer_overlay)
                        .map(|(name, _)| name.clone());

                    let peer_name = match peer_name {
                        Some(name) => name,
                        None => {
                            debug!(peer = %peer.address, "Peer not found in overlays");
                            continue;
                        }
                    };

                    // Get this peer's topology for depth check
                    let peer_topology = match topologies.get(&peer_name) {
                        Some(t) => t,
                        None => {
                            debug!(peer = %peer_name, "Peer topology not found");
                            continue;
                        }
                    };

                    // Check if chunk falls within this peer's depth
                    let proximity = chunk_address.proximity(&peer_overlay);
                    if proximity >= peer_topology.depth {
                        // Chunk is within this peer's replication responsibility
                        if !replicating_nodes.contains_key(&peer_name) {
                            replicating_nodes.insert(peer_name.clone(), peer_overlay);
                            nn_rep += 1;
                            debug!(
                                peer = %peer_name,
                                proximity = proximity,
                                depth = peer_topology.depth,
                                "Peer should replicate chunk"
                            );
                        }
                    }
                }
            }
        }

        info!(
            replicating_count = replicating_nodes.len(),
            nn_rep = nn_rep,
            "Found replicating nodes from closest node's neighbors"
        );

        Ok(replicating_nodes.keys().cloned().collect())
    }

    /// Calculate global replication factor by checking ALL nodes
    ///
    /// This matches beekeeper's GlobalReplicationFactor - count all nodes that have the chunk.
    async fn global_replication_factor(
        ctx: &CheckContext,
        chunk_reference: &str,
    ) -> Result<usize, CheckError> {
        let mut count = 0;

        for node in &ctx.nodes {
            match node.has_chunk(chunk_reference).await {
                Ok(true) => count += 1,
                Ok(false) => {}
                Err(e) => {
                    debug!(
                        node = ?node.name(),
                        error = %e,
                        "Error checking chunk for global RF"
                    );
                }
            }
        }

        Ok(count)
    }

    /// Generate random chunk data with random size
    fn generate_random_chunk(rng: &mut XorShiftRng) -> Vec<u8> {
        let payload_size = rng.next_usize(MAX_CHUNK_SIZE);
        let mut payload = vec![0u8; payload_size];
        rng.fill_bytes(&mut payload);
        payload
    }
}

#[async_trait]
impl Check for PullsyncCheck {
    fn name(&self) -> &'static str {
        "pullsync"
    }

    fn description(&self) -> &'static str {
        "Verify chunks are pulled/replicated to neighborhood nodes"
    }

    async fn run(
        &self,
        ctx: &CheckContext,
        opts: &CheckOptions,
    ) -> Result<CheckResult, CheckError> {
        let start = Instant::now();

        // Need at least 2 nodes for meaningful replication test
        if ctx.node_count() < 2 {
            return Err(CheckError::Config(
                "Pullsync check requires at least 2 nodes".to_string(),
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
        let replication_threshold: usize = opts
            .get_extra("replication_factor_threshold")
            .unwrap_or(DEFAULT_REPLICATION_FACTOR_THRESHOLD);

        info!(seed = seed, "Starting pullsync check");

        // Get overlays for ALL nodes (like beekeeper's FlattenOverlays)
        let overlays = Self::get_overlays(ctx).await?;
        info!(overlay_count = overlays.len(), "Got overlay addresses");

        // Get topologies for ALL nodes (like beekeeper's FlattenTopologies)
        let topologies = Self::get_topologies(ctx, &overlays).await?;
        info!(topology_count = topologies.len(), "Got topologies");

        // Batch options for creating batches
        let batch_opts = crate::client::BatchOptions::default();

        let mut all_results = Vec::new();
        let mut total_replication_factor = 0.0f64;

        // Use sorted node names like beekeeper
        let mut sorted_nodes: Vec<_> = ctx
            .nodes
            .iter()
            .filter_map(|n| n.name().map(|s| s.to_string()))
            .collect();
        sorted_nodes.sort();

        // Create per-node PRNGs (like beekeeper's PseudoGenerators)
        let mut master_rng = XorShiftRng::new(seed);
        let actual_upload_count = upload_node_count.min(sorted_nodes.len()).max(1);
        let mut node_rngs: Vec<XorShiftRng> = (0..actual_upload_count)
            .map(|_| master_rng.child())
            .collect();

        for upload_idx in 0..actual_upload_count {
            let upload_node_name = &sorted_nodes[upload_idx];

            // Find the upload node client
            let upload_node = ctx
                .nodes
                .iter()
                .find(|n| {
                    n.name()
                        .map(|name| name == upload_node_name)
                        .unwrap_or(false)
                })
                .ok_or_else(|| {
                    CheckError::Config(format!("Upload node {} not found", upload_node_name))
                })?;

            // Get or create a batch on this upload node
            let node_batch_id = match upload_node.get_or_create_batch(&batch_opts).await {
                Ok(batch_id) => {
                    info!(node = %upload_node_name, batch_id = %batch_id, "Got batch for uploads");
                    batch_id
                }
                Err(e) => {
                    return Err(CheckError::Failed(format!(
                        "node {}: created batched id {}",
                        upload_node_name, e
                    )));
                }
            };

            let rng = &mut node_rngs[upload_idx];

            for chunk_idx in 0..chunks_per_node {
                // Generate random chunk
                let payload = Self::generate_random_chunk(rng);
                let payload_bytes = Bytes::from(payload.clone());

                let chunk = match ContentChunk::new(payload_bytes) {
                    Ok(c) => c,
                    Err(e) => {
                        return Err(CheckError::Failed(format!(
                            "node {}: {}",
                            upload_node_name, e
                        )));
                    }
                };

                let chunk_address = *chunk.address();
                let chunk_address_hex = hex::encode(chunk_address.as_bytes());

                // Upload chunk
                let span = (payload.len() as u64).to_le_bytes();
                let mut chunk_data = Vec::with_capacity(8 + payload.len());
                chunk_data.extend_from_slice(&span);
                chunk_data.extend_from_slice(&payload);

                let uploaded_reference = match upload_node
                    .upload_chunk(&node_batch_id, chunk_data.clone())
                    .await
                {
                    Ok(resp) => resp.reference,
                    Err(e) => {
                        return Err(CheckError::Failed(format!(
                            "node {}: {}",
                            upload_node_name, e
                        )));
                    }
                };

                info!(chunk = %uploaded_reference, "Uploaded chunk");

                // Find closest node and log it (like beekeeper)
                let (closest_name, closest_overlay) =
                    Self::find_closest_node(&chunk_address, &overlays)
                        .ok_or_else(|| CheckError::Config("No closest node found".to_string()))?;

                info!(
                    upload_node = %upload_node_name,
                    chunk_idx = chunk_idx,
                    closest = %closest_name,
                    closest_overlay = %hex::encode(&closest_overlay.as_bytes()[..8]),
                    "Chunk closest node"
                );

                // Find replicating nodes using beekeeper's algorithm
                let replicating_nodes =
                    Self::find_replicating_nodes(ctx, &chunk_address, &overlays, &topologies)
                        .await?;

                if replicating_nodes.is_empty() {
                    info!(
                        upload_node = %upload_node_name,
                        chunk_idx = chunk_idx,
                        "Chunk does not have any designated replicators"
                    );
                    return Err(CheckError::Failed("pull sync".to_string()));
                }

                info!(
                    replicating_count = replicating_nodes.len(),
                    "Chunk should be on nodes within depth"
                );

                // Check each replicating node with retry logic matching beekeeper
                for replicating_name in &replicating_nodes {
                    // Find the client for this node
                    let node_client = ctx.nodes.iter().find(|n| {
                        n.name()
                            .map(|name| name == replicating_name)
                            .unwrap_or(false)
                    });

                    let node_client = match node_client {
                        Some(c) => c,
                        None => {
                            return Err(CheckError::Failed(format!(
                                "not found: {}",
                                replicating_name
                            )));
                        }
                    };

                    // Check with retries matching beekeeper's pattern:
                    // for t := 1; t < 5; t++ { time.Sleep(2 * t * second); ... }
                    let mut synced = false;
                    for t in 1..=RETRY_ATTEMPTS {
                        // Sleep BEFORE check (like beekeeper)
                        let delay = Duration::from_secs(2 * t as u64);
                        tokio::time::sleep(delay).await;

                        match node_client.has_chunk(&uploaded_reference).await {
                            Ok(true) => {
                                synced = true;
                                break;
                            }
                            Ok(false) => {
                                info!(
                                    upload_node = %upload_node_name,
                                    chunk_idx = chunk_idx,
                                    upload_overlay = %hex::encode(&overlays[upload_node_name].as_bytes()[..8]),
                                    chunk = %chunk_address_hex,
                                    pivot = %replicating_name,
                                    "Chunk not found on node. Retrying..."
                                );
                            }
                            Err(e) => {
                                return Err(CheckError::Failed(format!(
                                    "node {}: {}",
                                    replicating_name, e
                                )));
                            }
                        }
                    }

                    if !synced {
                        return Err(CheckError::Failed(format!(
                            "upload node {}. Chunk {} not found on node. Upload node: {} Chunk: {} Pivot: {}",
                            upload_node_name,
                            chunk_idx,
                            hex::encode(&overlays[upload_node_name].as_bytes()[..8]),
                            chunk_address_hex,
                            replicating_name
                        )));
                    }
                }

                // Calculate global replication factor (like beekeeper's GlobalReplicationFactor)
                let rf = Self::global_replication_factor(ctx, &uploaded_reference).await?;

                if rf < replication_threshold {
                    return Err(CheckError::Failed(format!(
                        "chunk {} has low replication factor. got {} want {}",
                        chunk_address_hex, rf, replication_threshold
                    )));
                }

                total_replication_factor += rf as f64;
                info!(replication_factor = rf, "Chunk replication factor");

                // Record success for all nodes that have the chunk
                for replicating_name in &replicating_nodes {
                    all_results.push(
                        NodeResult::passed(replicating_name)
                            .with_detail("action", "pullsync")
                            .with_detail("chunk", &chunk_address_hex)
                            .with_detail("replication_factor", rf),
                    );
                }
            }
        }

        let duration = start.elapsed();
        let total_chunks = (actual_upload_count * chunks_per_node) as f64;
        let avg_replication = if total_chunks > 0.0 {
            total_replication_factor / total_chunks
        } else {
            0.0
        };

        info!(
            avg_replication_factor = format!("{:.2}", avg_replication),
            "Done with average replication factor"
        );

        let message = format!("Average replication factor: {:.2}", avg_replication);

        Ok(CheckResult::new("pullsync", all_results, duration).with_message(message))
    }

    fn default_options(&self) -> CheckOptions {
        CheckOptions {
            timeout: Some(Duration::from_secs(600)), // 10 minutes - replication takes time
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
        let check = PullsyncCheck;
        assert_eq!(check.name(), "pullsync");
        assert!(!check.description().is_empty());
    }

    #[test]
    fn test_default_options() {
        let check = PullsyncCheck;
        let opts = check.default_options();
        assert_eq!(opts.timeout, Some(Duration::from_secs(600)));
    }

    #[test]
    fn test_find_closest_node() {
        let mut overlays = HashMap::new();

        // Target chunk address: 0x1000...
        let chunk = SwarmAddress::new([0x10; 32]);

        // Node A: close to chunk
        let close = SwarmAddress::new([0x11; 32]);
        overlays.insert("node-a".to_string(), close);

        // Node B: far from chunk
        let far = SwarmAddress::new([0xF0; 32]);
        overlays.insert("node-b".to_string(), far);

        let (name, _) = PullsyncCheck::find_closest_node(&chunk, &overlays).unwrap();
        assert_eq!(name, "node-a");
    }

    #[test]
    fn test_prng_deterministic() {
        let mut rng1 = XorShiftRng::new(12345);
        let mut rng2 = XorShiftRng::new(12345);

        for _ in 0..100 {
            assert_eq!(rng1.next(), rng2.next());
        }
    }

    #[test]
    fn test_prng_child_independence() {
        let mut master = XorShiftRng::new(12345);
        let mut child1 = master.child();
        let mut child2 = master.child();

        // Children should produce different sequences
        assert_ne!(child1.next(), child2.next());
    }

    #[test]
    fn test_retry_timing() {
        // Verify our retry timing matches beekeeper's pattern
        // beekeeper: for t := 1; t < 5; t++ { time.Sleep(2 * t * second) }
        // This means delays of 2s, 4s, 6s, 8s
        let expected_delays: Vec<u64> = (1..=RETRY_ATTEMPTS).map(|t| 2 * t as u64).collect();
        assert_eq!(expected_delays, vec![2, 4, 6, 8]);
    }
}
