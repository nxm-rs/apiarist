//! Apiarist Test Kit
//!
//! Test infrastructure and utilities for Swarm Bee testing.
//!
//! This crate provides:
//! - Seeded random data generation for reproducible tests
//! - Mock clients for unit testing
//!
//! For chunk utilities and proximity calculations, use `nectar_primitives` directly:
//! - `nectar_primitives::ContentChunk` - proper BMT-hashed chunks
//! - `nectar_primitives::SwarmAddress` - proximity and XOR distance
//!
//! # Example
//!
//! ```rust
//! use apiarist_testkit::random::PseudoGenerator;
//!
//! // Create reproducible random generator
//! let mut rng = PseudoGenerator::new(12345);
//!
//! // Generate random data
//! let data = rng.random_bytes(4096);
//! ```

pub mod mock;
pub mod random;

// Re-exports for convenience
pub use random::PseudoGenerator;
