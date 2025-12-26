//! Kademlia proximity and XOR distance utilities
//!
//! Swarm uses Kademlia DHT for content addressing. The "proximity" between
//! two addresses is based on XOR distance - the number of leading zero bits
//! when XORing the two addresses together.
//!
//! Higher proximity = closer in Kademlia space = more likely to store each other's data.
//!
//! # Example
//!
//! ```rust
//! use apiarist_testkit::proximity::{proximity, xor_distance};
//!
//! let addr1 = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
//! let addr2 = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcde0";
//!
//! let prox = proximity(addr1, addr2);
//! println!("Proximity: {} bits", prox);
//! ```

use thiserror::Error;

/// Errors that can occur during proximity calculations
#[derive(Debug, Error)]
pub enum ProximityError {
    #[error("Invalid hex string: {0}")]
    InvalidHex(String),

    #[error("Address length mismatch: {0} vs {1}")]
    LengthMismatch(usize, usize),
}

/// Calculate the proximity (leading zero bits in XOR) between two hex addresses
///
/// Returns the number of leading zero bits when the two addresses are XORed together.
/// Higher values mean the addresses are "closer" in Kademlia space.
///
/// # Arguments
///
/// * `a` - First address as hex string (with or without 0x prefix)
/// * `b` - Second address as hex string (with or without 0x prefix)
///
/// # Returns
///
/// The proximity value (0-256 for 32-byte addresses)
pub fn proximity(a: &str, b: &str) -> u8 {
    match proximity_checked(a, b) {
        Ok(p) => p,
        Err(_) => 0, // Return 0 for invalid inputs (like beekeeper does)
    }
}

/// Calculate proximity with error handling
pub fn proximity_checked(a: &str, b: &str) -> Result<u8, ProximityError> {
    let a = strip_hex_prefix(a);
    let b = strip_hex_prefix(b);

    let a_bytes = hex::decode(a).map_err(|e| ProximityError::InvalidHex(e.to_string()))?;
    let b_bytes = hex::decode(b).map_err(|e| ProximityError::InvalidHex(e.to_string()))?;

    if a_bytes.len() != b_bytes.len() {
        return Err(ProximityError::LengthMismatch(a_bytes.len(), b_bytes.len()));
    }

    Ok(proximity_bytes(&a_bytes, &b_bytes))
}

/// Calculate proximity from raw bytes
///
/// Returns the number of leading zero bits in the XOR of the two byte slices.
/// Maximum value is capped at 255 (u8::MAX) even for identical inputs.
pub fn proximity_bytes(a: &[u8], b: &[u8]) -> u8 {
    let mut proximity: u16 = 0; // Use u16 to avoid overflow

    for (byte_a, byte_b) in a.iter().zip(b.iter()) {
        let xor = byte_a ^ byte_b;
        if xor == 0 {
            proximity += 8;
        } else {
            proximity += xor.leading_zeros() as u16;
            break;
        }
    }

    // Cap at u8::MAX (255) - identical 32-byte addresses would have proximity 256
    proximity.min(255) as u8
}

/// Calculate the full XOR distance between two hex addresses
///
/// Returns the XOR of the two addresses as a byte vector.
pub fn xor_distance(a: &str, b: &str) -> Result<Vec<u8>, ProximityError> {
    let a = strip_hex_prefix(a);
    let b = strip_hex_prefix(b);

    let a_bytes = hex::decode(a).map_err(|e| ProximityError::InvalidHex(e.to_string()))?;
    let b_bytes = hex::decode(b).map_err(|e| ProximityError::InvalidHex(e.to_string()))?;

    if a_bytes.len() != b_bytes.len() {
        return Err(ProximityError::LengthMismatch(a_bytes.len(), b_bytes.len()));
    }

    Ok(a_bytes
        .iter()
        .zip(b_bytes.iter())
        .map(|(x, y)| x ^ y)
        .collect())
}

/// Check if address `a` is closer to `target` than address `b`
///
/// Returns true if `a` has higher proximity to `target` than `b`.
pub fn is_closer(target: &str, a: &str, b: &str) -> bool {
    proximity(target, a) > proximity(target, b)
}

/// Find the closest address to a target from a list of addresses
///
/// Returns the address with the highest proximity to the target,
/// along with its proximity value.
pub fn closest_to<'a>(target: &str, addresses: &[&'a str]) -> Option<(&'a str, u8)> {
    addresses
        .iter()
        .map(|&addr| (addr, proximity(target, addr)))
        .max_by_key(|(_, prox)| *prox)
}

fn strip_hex_prefix(s: &str) -> &str {
    s.strip_prefix("0x").unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_addresses() {
        let addr = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        // XOR of identical values is all zeros = maximum proximity (capped at 255)
        assert_eq!(proximity(addr, addr), 255);
    }

    #[test]
    fn test_completely_different() {
        let a = "0000000000000000000000000000000000000000000000000000000000000000";
        let b = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        // First bit differs = proximity 0
        assert_eq!(proximity(a, b), 0);
    }

    #[test]
    fn test_one_bit_difference() {
        let a = "8000000000000000000000000000000000000000000000000000000000000000";
        let b = "0000000000000000000000000000000000000000000000000000000000000000";
        // First bit (MSB) differs
        assert_eq!(proximity(a, b), 0);

        let c = "4000000000000000000000000000000000000000000000000000000000000000";
        // Second bit differs
        assert_eq!(proximity(b, c), 1);
    }

    #[test]
    fn test_proximity_with_prefix() {
        let a = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        let b = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        assert_eq!(proximity(a, b), 255);
    }

    #[test]
    fn test_is_closer() {
        let target = "1000000000000000000000000000000000000000000000000000000000000000";
        let close = "1100000000000000000000000000000000000000000000000000000000000000";
        let far = "f000000000000000000000000000000000000000000000000000000000000000";

        assert!(is_closer(target, close, far));
        assert!(!is_closer(target, far, close));
    }

    #[test]
    fn test_closest_to() {
        let target = "1000000000000000000000000000000000000000000000000000000000000000";
        let addresses = vec![
            "f000000000000000000000000000000000000000000000000000000000000000",
            "1100000000000000000000000000000000000000000000000000000000000000",
            "1010000000000000000000000000000000000000000000000000000000000000",
        ];

        let (closest, prox) = closest_to(target, &addresses).unwrap();
        assert_eq!(
            closest,
            "1010000000000000000000000000000000000000000000000000000000000000"
        );
        assert!(prox > 0);
    }

    #[test]
    fn test_xor_distance() {
        let a = "ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00";
        let b = "00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff";

        let distance = xor_distance(a, b).unwrap();
        // XOR should be all 0xff
        assert!(distance.iter().all(|&x| x == 0xff));
    }

    #[test]
    fn test_invalid_hex() {
        let result = proximity_checked("not_hex", "also_not_hex");
        assert!(result.is_err());
    }

    #[test]
    fn test_length_mismatch() {
        let result = proximity_checked("1234", "123456");
        assert!(result.is_err());
    }
}
