//! Chunk utilities for Swarm content addressing
//!
//! Swarm content is divided into chunks of up to 4096 bytes. Each chunk
//! is addressed by the hash of its content. This module provides utilities
//! for working with chunks and calculating their addresses.
//!
//! # Chunk Address Calculation
//!
//! The chunk address is calculated using the BMT (Binary Merkle Tree) hash:
//! 1. Content is padded to 4096 bytes if smaller
//! 2. BMT hash is computed over the padded content
//! 3. The resulting 32-byte hash is the chunk address
//!
//! For simplicity, this implementation uses Keccak-256 directly.
//! The full BMT implementation matches the Bee node's behavior.

use sha3::{Digest, Keccak256};
use std::collections::HashMap;

use crate::proximity::{closest_to, proximity};

/// Maximum chunk payload size (4KB)
pub const CHUNK_MAX_SIZE: usize = 4096;

/// Swarm address length (32 bytes = 64 hex chars)
pub const ADDRESS_LENGTH: usize = 32;

/// A Swarm chunk with content and computed address
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Raw chunk data
    data: Vec<u8>,
    /// Computed chunk address (hex string)
    address: String,
}

impl Chunk {
    /// Create a new chunk from data
    ///
    /// The address is automatically calculated from the content.
    pub fn new(data: Vec<u8>) -> Self {
        let address = calculate_chunk_address(&data);
        Self { data, address }
    }

    /// Create a chunk from data with a pre-calculated address
    ///
    /// Use this when you already know the address (e.g., from an upload response).
    pub fn with_address(data: Vec<u8>, address: String) -> Self {
        Self { data, address }
    }

    /// Get the chunk data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get the chunk size
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Get the chunk address (hex string, no 0x prefix)
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Check if the chunk address matches another address
    pub fn address_matches(&self, other: &str) -> bool {
        let other = other.strip_prefix("0x").unwrap_or(other);
        self.address.eq_ignore_ascii_case(other)
    }

    /// Calculate proximity to another address
    pub fn proximity_to(&self, other: &str) -> u8 {
        proximity(&self.address, other)
    }

    /// Find the closest node to this chunk from a map of node names to overlay addresses
    ///
    /// Returns the node name and overlay address of the closest node.
    pub fn closest_node(&self, overlays: &HashMap<String, String>) -> Option<(String, String)> {
        let addrs: Vec<&str> = overlays.values().map(|s| s.as_str()).collect();
        let (closest_addr, _) = closest_to(&self.address, &addrs)?;

        overlays
            .iter()
            .find(|(_, addr)| addr.as_str() == closest_addr)
            .map(|(name, addr)| (name.clone(), addr.clone()))
    }

    /// Find the closest node from a slice of (name, overlay) pairs
    pub fn closest_node_from_slice(&self, nodes: &[(&str, &str)]) -> Option<(String, String, u8)> {
        nodes
            .iter()
            .map(|(name, overlay)| (name.to_string(), overlay.to_string(), proximity(&self.address, overlay)))
            .max_by_key(|(_, _, prox)| *prox)
    }
}

/// Calculate the address of a chunk from its content
///
/// This uses a simplified hash calculation. The full Bee implementation
/// uses BMT (Binary Merkle Tree) hashing with span prefixing.
///
/// For test purposes, this provides deterministic addresses that can
/// be used to verify chunk routing and storage.
pub fn calculate_chunk_address(data: &[u8]) -> String {
    // Simplified: just hash the data directly
    // Full implementation would use BMT with span
    let mut hasher = Keccak256::new();

    // Include length prefix (like Bee's span)
    let len = data.len() as u64;
    hasher.update(len.to_le_bytes());
    hasher.update(data);

    let result = hasher.finalize();
    hex::encode(result)
}

/// Calculate BMT root hash for chunk data
///
/// This is a simplified BMT implementation for testing.
/// Production code should use the full BMT algorithm matching Bee.
pub fn bmt_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    let len = data.len() as u64;
    hasher.update(len.to_le_bytes());
    hasher.update(data);
    hasher.finalize().into()
}

/// Generate a random chunk with the given seed
pub fn random_chunk(seed: u64) -> Chunk {
    use crate::random::PseudoGenerator;

    let mut rng = PseudoGenerator::new(seed);
    let data = rng.random_chunk_data(CHUNK_MAX_SIZE);
    Chunk::new(data)
}

/// Generate multiple random chunks
pub fn random_chunks(seed: u64, count: usize) -> Vec<Chunk> {
    use crate::random::pseudo_generators;

    let rngs = pseudo_generators(seed, count);
    rngs.into_iter()
        .map(|mut rng| {
            let data = rng.random_chunk_data(CHUNK_MAX_SIZE);
            Chunk::new(data)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_creation() {
        let data = b"hello world".to_vec();
        let chunk = Chunk::new(data.clone());

        assert_eq!(chunk.data(), data.as_slice());
        assert_eq!(chunk.size(), 11);
        assert_eq!(chunk.address().len(), 64); // 32 bytes = 64 hex chars
    }

    #[test]
    fn test_deterministic_address() {
        let data = b"deterministic test data".to_vec();

        let chunk1 = Chunk::new(data.clone());
        let chunk2 = Chunk::new(data);

        assert_eq!(chunk1.address(), chunk2.address());
    }

    #[test]
    fn test_different_data_different_address() {
        let chunk1 = Chunk::new(b"data 1".to_vec());
        let chunk2 = Chunk::new(b"data 2".to_vec());

        assert_ne!(chunk1.address(), chunk2.address());
    }

    #[test]
    fn test_address_matches() {
        let chunk = Chunk::new(b"test".to_vec());
        let addr = chunk.address().to_string();

        assert!(chunk.address_matches(&addr));
        assert!(chunk.address_matches(&format!("0x{}", addr)));
        assert!(chunk.address_matches(&addr.to_uppercase()));
    }

    #[test]
    fn test_closest_node() {
        let chunk = Chunk::new(b"find me".to_vec());

        let mut overlays = HashMap::new();
        overlays.insert(
            "node-0".to_string(),
            "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        );
        overlays.insert(
            "node-1".to_string(),
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string(),
        );

        let (name, _addr) = chunk.closest_node(&overlays).unwrap();
        // One of them should be closest
        assert!(name == "node-0" || name == "node-1");
    }

    #[test]
    fn test_random_chunk() {
        let chunk1 = random_chunk(42);
        let chunk2 = random_chunk(42);

        // Same seed = same chunk
        assert_eq!(chunk1.data(), chunk2.data());
        assert_eq!(chunk1.address(), chunk2.address());

        // Different seed = different chunk
        let chunk3 = random_chunk(43);
        assert_ne!(chunk1.data(), chunk3.data());
    }

    #[test]
    fn test_random_chunks() {
        let chunks = random_chunks(42, 5);
        assert_eq!(chunks.len(), 5);

        // All chunks should have different addresses
        let addresses: std::collections::HashSet<_> =
            chunks.iter().map(|c| c.address().to_string()).collect();
        assert_eq!(addresses.len(), 5);
    }
}
