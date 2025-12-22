//! Apiarist - The Beekeeper Who Stress-Tests Your Swarm
//!
//! A Rust-based testing tool for Ethereum Swarm networks that runs *inside*
//! your test environment instead of awkwardly poking at it from outside.
//!
//! ## Architecture
//!
//! Apiarist follows the [Assertoor](https://github.com/ethpandaops/assertoor) pattern:
//! - Runs as a service within Kurtosis test environments
//! - Executes YAML-defined test configurations
//! - Provides HTTP status API for monitoring progress
//! - Exports Prometheus metrics for observability
//!
//! ## Modules
//!
//! - [`client`] - HTTP client for Bee node API
//! - `checks` - Check implementations (pingpong, peercount, kademlia, etc.)
//! - `config` - Configuration parsing (cluster, checks)
//! - `orchestrator` - Task scheduling and execution
//! - `api` - Status HTTP API
//! - `metrics` - Prometheus metrics

pub mod api;
pub mod checks;
pub mod client;
pub mod config;
