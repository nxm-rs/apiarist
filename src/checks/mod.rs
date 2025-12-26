//! Check implementations for Bee cluster testing
//!
//! This module provides the `Check` trait and implementations for various
//! cluster health and functionality checks.
//!
//! ## Check Categories
//!
//! - **Core Protocol (P0)**: pingpong, peercount, kademlia, fullconnectivity
//! - **Content (P1)**: retrieval, pushsync, pullsync (TODO)
//! - **Economic (P3)**: postage, settlements, balances (TODO)
//! - **Long-running (P4)**: smoke, load (TODO)
//!
//! ## Adding New Checks
//!
//! 1. Create a new file in `src/checks/` (e.g., `mycheck.rs`)
//! 2. Implement the `Check` trait
//! 3. Register in `registry.rs`
//! 4. Add to `mod.rs` exports

mod fullconnectivity;
mod kademlia;
mod peercount;
mod pingpong;
pub mod registry;
mod traits;

pub use fullconnectivity::FullconnectivityCheck;
pub use kademlia::KademliaCheck;
pub use peercount::PeercountCheck;
pub use pingpong::PingpongCheck;
pub use registry::CHECKS;
pub use traits::*;
