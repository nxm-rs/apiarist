//! Apiarist Test Kit
//!
//! Test infrastructure and utilities for Swarm Bee testing.
//!
//! This crate provides:
//! - Seeded random data generation for reproducible tests
//! - Chunk utilities (address calculation, proximity/XOR distance)
//! - Mock clients for unit testing
//!
//! # Example
//!
//! ```rust
//! use apiarist_testkit::random::PseudoGenerator;
//! use apiarist_testkit::chunk::Chunk;
//!
//! // Create reproducible random generator
//! let mut rng = PseudoGenerator::new(12345);
//!
//! // Generate random chunk data
//! let data = rng.random_bytes(4096);
//! let chunk = Chunk::new(data);
//!
//! println!("Chunk address: {}", chunk.address());
//! ```

pub mod chunk;
pub mod mock;
pub mod proximity;
pub mod random;

// Re-exports for convenience
pub use chunk::Chunk;
pub use proximity::{proximity, xor_distance};
pub use random::PseudoGenerator;
