//! Apiary - Swarm Node Testing Framework
//!
//! A Rust-based testing tool for Ethereum Swarm networks, designed to run
//! comprehensive checks against Bee node clusters.
//!
//! ## Architecture
//!
//! Apiary follows the [Assertoor](https://github.com/ethpandaops/assertoor) pattern:
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
