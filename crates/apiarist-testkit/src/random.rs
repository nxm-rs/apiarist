//! Seeded random data generation
//!
//! Provides reproducible random data generation for tests.
//! Using the same seed produces identical sequences of random data,
//! making tests deterministic and failures reproducible.
//!
//! # Example
//!
//! ```rust
//! use apiarist_testkit::random::PseudoGenerator;
//!
//! let mut rng1 = PseudoGenerator::new(42);
//! let mut rng2 = PseudoGenerator::new(42);
//!
//! // Same seed produces same output
//! assert_eq!(rng1.random_bytes(100), rng2.random_bytes(100));
//! ```

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

/// Seeded pseudo-random generator for reproducible test data
#[derive(Debug)]
pub struct PseudoGenerator {
    rng: StdRng,
    seed: u64,
}

impl PseudoGenerator {
    /// Create a new generator with the given seed
    pub fn new(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
            seed,
        }
    }

    /// Get the seed used to create this generator
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Generate random bytes of the specified length
    pub fn random_bytes(&mut self, len: usize) -> Vec<u8> {
        let mut bytes = vec![0u8; len];
        self.rng.fill(&mut bytes[..]);
        bytes
    }

    /// Generate a random chunk of data (1 to max_size bytes)
    ///
    /// The size is randomly chosen between 1 and max_size (inclusive).
    pub fn random_chunk_data(&mut self, max_size: usize) -> Vec<u8> {
        let size = self.rng.random_range(1..=max_size);
        self.random_bytes(size)
    }

    /// Generate random bytes filling a fixed-size array
    pub fn random_array<const N: usize>(&mut self) -> [u8; N] {
        let mut arr = [0u8; N];
        self.rng.fill(&mut arr);
        arr
    }

    /// Generate a random u64
    pub fn random_u64(&mut self) -> u64 {
        self.rng.random()
    }

    /// Generate a random usize in the given range
    pub fn random_range(&mut self, range: std::ops::Range<usize>) -> usize {
        self.rng.random_range(range)
    }
}

/// Create multiple pseudo-random generators from a single seed
///
/// This is useful when you need multiple independent random streams
/// that are still reproducible from a single seed.
///
/// # Example
///
/// ```rust
/// use apiarist_testkit::random::pseudo_generators;
///
/// let rngs = pseudo_generators(42, 5);
/// assert_eq!(rngs.len(), 5);
/// ```
pub fn pseudo_generators(seed: u64, count: usize) -> Vec<PseudoGenerator> {
    // Use the initial seed to generate seeds for each generator
    let mut master_rng = StdRng::seed_from_u64(seed);
    (0..count)
        .map(|_| PseudoGenerator::new(master_rng.random()))
        .collect()
}

/// Generate a random seed (for when you don't care about reproducibility)
pub fn random_seed() -> u64 {
    rand::random()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reproducibility() {
        let mut rng1 = PseudoGenerator::new(12345);
        let mut rng2 = PseudoGenerator::new(12345);

        let bytes1 = rng1.random_bytes(1000);
        let bytes2 = rng2.random_bytes(1000);

        assert_eq!(bytes1, bytes2);
    }

    #[test]
    fn test_different_seeds_different_output() {
        let mut rng1 = PseudoGenerator::new(1);
        let mut rng2 = PseudoGenerator::new(2);

        let bytes1 = rng1.random_bytes(100);
        let bytes2 = rng2.random_bytes(100);

        assert_ne!(bytes1, bytes2);
    }

    #[test]
    fn test_pseudo_generators() {
        let rngs1 = pseudo_generators(42, 3);
        let rngs2 = pseudo_generators(42, 3);

        // Same seed produces same sequence of generator seeds
        for (mut r1, mut r2) in rngs1.into_iter().zip(rngs2.into_iter()) {
            assert_eq!(r1.random_bytes(50), r2.random_bytes(50));
        }
    }

    #[test]
    fn test_random_chunk_data() {
        let mut rng = PseudoGenerator::new(42);

        for _ in 0..100 {
            let data = rng.random_chunk_data(4096);
            assert!(!data.is_empty());
            assert!(data.len() <= 4096);
        }
    }

    #[test]
    fn test_seed_getter() {
        let rng = PseudoGenerator::new(999);
        assert_eq!(rng.seed(), 999);
    }
}
