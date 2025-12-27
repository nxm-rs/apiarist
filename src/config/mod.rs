//! Configuration parsing
//!
//! Handles parsing of cluster configuration files and check options.
//!
//! ## Configuration Format
//!
//! ```yaml
//! cluster:
//!   name: my-cluster
//!   bootnode:
//!     name: bootnode
//!     api_url: http://bootnode:1633
//!   nodes:
//!     - name: bee-0
//!       api_url: http://bee-0:1633
//!     - name: bee-1
//!       api_url: http://bee-1:1633
//!
//! checks:
//!   pingpong:
//!     enabled: true
//!     timeout: 5m
//!     retries: 5
//!
//!   peercount:
//!     enabled: true
//!     min_peers: 1
//! ```

mod cluster;

pub use cluster::{CheckConfig, ClusterConfig, Config, NodeConfig, NodeType};
