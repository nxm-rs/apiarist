//! Check implementations for Bee cluster testing
//!
//! This module provides the `Check` trait and implementations for various
//! cluster health and functionality checks.
//!
//! ## Check Categories
//!
//! - **Core Protocol**: pingpong, peercount, kademlia, fullconnectivity
//! - **Content**: retrieval, pushsync, pullsync
//! - **Economic**: postage, settlements, balances
//! - **Long-running**: smoke, load
//!
//! ## Adding New Checks
//!
//! 1. Create a new file in `src/checks/` (e.g., `mycheck.rs`)
//! 2. Implement the `Check` trait
//! 3. Register in `CHECKS` registry below
//! 4. Add to `mod.rs` exports

mod pingpong;
pub mod registry;
mod traits;

pub use pingpong::PingpongCheck;
pub use registry::CHECKS;
pub use traits::*;
