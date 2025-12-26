//! Mock client for unit testing
//!
//! Provides mock implementations of Bee client responses for testing
//! checks without requiring actual Bee nodes.
//!
//! # Example
//!
//! ```rust
//! use apiarist_testkit::mock::{MockNode, MockCluster};
//!
//! // Create a mock cluster
//! let mut cluster = MockCluster::new();
//! cluster.add_node(MockNode::new("bee-0", "abc123..."));
//! cluster.add_node(MockNode::new("bee-1", "def456..."));
//!
//! // Use in tests
//! assert_eq!(cluster.node_count(), 2);
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock representation of a Bee node for testing
#[derive(Debug, Clone)]
pub struct MockNode {
    /// Node name/identifier
    pub name: String,
    /// Overlay address (hex string)
    pub overlay: String,
    /// List of peer overlay addresses
    pub peers: Vec<String>,
    /// Whether the node is healthy
    pub healthy: bool,
    /// Kademlia depth
    pub depth: u8,
    /// Connected peer count
    pub connected: usize,
    /// Simulated RTT for pingpong (in nanoseconds as string)
    pub rtt: String,
    /// Whether pingpong should succeed
    pub pingpong_success: bool,
    /// Stored chunks (address -> data)
    pub chunks: HashMap<String, Vec<u8>>,
}

impl MockNode {
    /// Create a new mock node with default settings
    pub fn new(name: impl Into<String>, overlay: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            overlay: overlay.into(),
            peers: Vec::new(),
            healthy: true,
            depth: 8,
            connected: 0,
            rtt: "1000000".to_string(), // 1ms
            pingpong_success: true,
            chunks: HashMap::new(),
        }
    }

    /// Add a peer to this node
    pub fn with_peer(mut self, peer_overlay: impl Into<String>) -> Self {
        let peer = peer_overlay.into();
        self.peers.push(peer);
        self.connected = self.peers.len();
        self
    }

    /// Add multiple peers
    pub fn with_peers(mut self, peers: Vec<String>) -> Self {
        self.connected = peers.len();
        self.peers = peers;
        self
    }

    /// Set health status
    pub fn with_health(mut self, healthy: bool) -> Self {
        self.healthy = healthy;
        self
    }

    /// Set Kademlia depth
    pub fn with_depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// Set pingpong behavior
    pub fn with_pingpong(mut self, success: bool, rtt: impl Into<String>) -> Self {
        self.pingpong_success = success;
        self.rtt = rtt.into();
        self
    }

    /// Store a chunk
    pub fn store_chunk(&mut self, address: impl Into<String>, data: Vec<u8>) {
        self.chunks.insert(address.into(), data);
    }

    /// Check if node has a chunk
    pub fn has_chunk(&self, address: &str) -> bool {
        self.chunks.contains_key(address)
    }
}

/// Mock cluster for testing
#[derive(Debug, Default)]
pub struct MockCluster {
    /// Bootnode
    pub bootnode: Option<MockNode>,
    /// Regular nodes
    pub nodes: Vec<MockNode>,
}

impl MockCluster {
    /// Create a new empty mock cluster
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the bootnode
    pub fn with_bootnode(mut self, node: MockNode) -> Self {
        self.bootnode = Some(node);
        self
    }

    /// Add a node to the cluster
    pub fn add_node(&mut self, node: MockNode) {
        self.nodes.push(node);
    }

    /// Add a node (builder pattern)
    pub fn with_node(mut self, node: MockNode) -> Self {
        self.nodes.push(node);
        self
    }

    /// Get all nodes (including bootnode)
    pub fn all_nodes(&self) -> Vec<&MockNode> {
        let mut nodes = Vec::new();
        if let Some(ref bn) = self.bootnode {
            nodes.push(bn);
        }
        nodes.extend(self.nodes.iter());
        nodes
    }

    /// Get total node count
    pub fn node_count(&self) -> usize {
        self.bootnode.iter().count() + self.nodes.len()
    }

    /// Get all overlay addresses
    pub fn overlays(&self) -> HashMap<String, String> {
        self.all_nodes()
            .iter()
            .map(|n| (n.name.clone(), n.overlay.clone()))
            .collect()
    }

    /// Create a fully connected cluster
    ///
    /// Each node will have all other nodes as peers.
    pub fn fully_connected(mut self) -> Self {
        let all_overlays: Vec<String> = self.all_nodes().iter().map(|n| n.overlay.clone()).collect();

        if let Some(ref mut bn) = self.bootnode {
            bn.peers = all_overlays
                .iter()
                .filter(|o| o != &&bn.overlay)
                .cloned()
                .collect();
            bn.connected = bn.peers.len();
        }

        for node in &mut self.nodes {
            node.peers = all_overlays
                .iter()
                .filter(|o| o != &&node.overlay)
                .cloned()
                .collect();
            node.connected = node.peers.len();
        }

        self
    }
}

/// Create a standard test cluster with N nodes
///
/// Creates a bootnode plus N worker nodes, all healthy and connected.
pub fn create_test_cluster(node_count: usize) -> MockCluster {
    let mut cluster = MockCluster::new();

    // Generate deterministic overlay addresses
    let mut overlays = Vec::new();
    for i in 0..=node_count {
        let overlay = format!("{:064x}", (i as u64) * 0x1111111111111111u64);
        overlays.push(overlay);
    }

    // Create bootnode
    let bootnode = MockNode::new("bootnode", overlays[0].clone());
    cluster = cluster.with_bootnode(bootnode);

    // Create worker nodes
    for i in 0..node_count {
        let node = MockNode::new(format!("bee-{}", i), overlays[i + 1].clone());
        cluster.add_node(node);
    }

    // Make fully connected
    cluster.fully_connected()
}

/// Thread-safe mock state for async testing
#[derive(Debug, Default)]
pub struct AsyncMockState {
    /// Pingpong call count per peer
    pub pingpong_calls: RwLock<HashMap<String, usize>>,
    /// Chunk upload count
    pub upload_count: RwLock<usize>,
    /// Chunk download count
    pub download_count: RwLock<usize>,
}

impl AsyncMockState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub async fn record_pingpong(&self, peer: &str) {
        let mut calls = self.pingpong_calls.write().await;
        *calls.entry(peer.to_string()).or_insert(0) += 1;
    }

    pub async fn record_upload(&self) {
        let mut count = self.upload_count.write().await;
        *count += 1;
    }

    pub async fn record_download(&self) {
        let mut count = self.download_count.write().await;
        *count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_node_creation() {
        let node = MockNode::new("test-node", "abc123");
        assert_eq!(node.name, "test-node");
        assert_eq!(node.overlay, "abc123");
        assert!(node.healthy);
        assert!(node.peers.is_empty());
    }

    #[test]
    fn test_mock_node_builder() {
        let node = MockNode::new("test", "overlay")
            .with_peer("peer1")
            .with_peer("peer2")
            .with_depth(10)
            .with_health(true);

        assert_eq!(node.peers.len(), 2);
        assert_eq!(node.connected, 2);
        assert_eq!(node.depth, 10);
    }

    #[test]
    fn test_mock_cluster() {
        let cluster = MockCluster::new()
            .with_bootnode(MockNode::new("bootnode", "bn-overlay"))
            .with_node(MockNode::new("bee-0", "overlay-0"))
            .with_node(MockNode::new("bee-1", "overlay-1"));

        assert_eq!(cluster.node_count(), 3);
        assert_eq!(cluster.all_nodes().len(), 3);
    }

    #[test]
    fn test_fully_connected() {
        let cluster = create_test_cluster(3);

        // Each node should have all others as peers
        for node in cluster.all_nodes() {
            assert_eq!(node.peers.len(), 3); // 3 other nodes
            assert!(node.connected > 0);
        }
    }

    #[test]
    fn test_chunk_storage() {
        let mut node = MockNode::new("test", "overlay");
        node.store_chunk("chunk-addr", b"data".to_vec());

        assert!(node.has_chunk("chunk-addr"));
        assert!(!node.has_chunk("other-addr"));
    }

    #[tokio::test]
    async fn test_async_mock_state() {
        let state = AsyncMockState::new();

        state.record_pingpong("peer1").await;
        state.record_pingpong("peer1").await;
        state.record_pingpong("peer2").await;

        let calls = state.pingpong_calls.read().await;
        assert_eq!(calls.get("peer1"), Some(&2));
        assert_eq!(calls.get("peer2"), Some(&1));
    }
}
