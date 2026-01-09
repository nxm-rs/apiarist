//! Check implementations for Bee cluster testing
//!
//! This module provides the `Check` trait and implementations for various
//! cluster health and functionality checks.
//!
//! ## Check Categories
//!
//! - **Core Protocol (P0)**: pingpong, peercount, fullconnectivity
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
mod peercount;
mod pingpong;
mod pullsync;
mod pushsync;
pub mod registry;
mod retrieval;
mod smoke;
mod soc;
mod traits;

pub use fullconnectivity::FullconnectivityCheck;
pub use peercount::PeercountCheck;
pub use pingpong::PingpongCheck;
pub use pullsync::PullsyncCheck;
pub use pushsync::PushsyncCheck;
pub use registry::CHECKS;
pub use retrieval::RetrievalCheck;
pub use smoke::SmokeCheck;
pub use soc::SocCheck;
pub use traits::*;
