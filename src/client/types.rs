//! Bee API Types
//!
//! These types are derived from the OpenAPI specification:
//! - `openapi/Swarm.yaml` - API endpoints
//! - `openapi/SwarmCommon.yaml` - Shared schemas
//!
//! When adding new types, reference the OpenAPI spec for the correct structure.

use serde::{Deserialize, Serialize};

/// Swarm overlay address (64 hex characters)
/// OpenAPI: SwarmCommon.yaml#/components/schemas/SwarmAddress
pub type SwarmAddress = String;

/// Ethereum address (40 hex characters with 0x prefix)
/// OpenAPI: SwarmCommon.yaml#/components/schemas/EthereumAddress
pub type EthereumAddress = String;

/// Public key (66 hex characters)
/// OpenAPI: SwarmCommon.yaml#/components/schemas/PublicKey
pub type PublicKey = String;

/// P2P underlay address (multiaddr format)
/// OpenAPI: SwarmCommon.yaml#/components/schemas/P2PUnderlay
pub type P2PUnderlay = String;

/// Duration in nanoseconds as string
/// OpenAPI: SwarmCommon.yaml#/components/schemas/Duration
pub type Duration = String;

/// Node addresses response
/// OpenAPI: SwarmCommon.yaml#/components/schemas/Addresses
/// Endpoint: GET /addresses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Addresses {
    /// Overlay address of the node
    pub overlay: SwarmAddress,
    /// Underlay addresses (libp2p multiaddrs)
    #[serde(default)]
    pub underlay: Vec<P2PUnderlay>,
    /// Ethereum address
    #[serde(default)]
    pub ethereum: Option<EthereumAddress>,
    /// Blockchain address (may differ from ethereum in some configs)
    #[serde(default)]
    pub chain_address: Option<EthereumAddress>,
    /// Node's public key
    #[serde(default)]
    pub public_key: Option<PublicKey>,
    /// PSS public key
    #[serde(default)]
    pub pss_public_key: Option<PublicKey>,
}

/// Single peer address wrapper
/// OpenAPI: SwarmCommon.yaml#/components/schemas/Address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub address: SwarmAddress,
}

/// List of connected peers
/// OpenAPI: SwarmCommon.yaml#/components/schemas/Peers
/// Endpoint: GET /peers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peers {
    pub peers: Vec<Address>,
}

/// Pingpong response with round-trip time
/// OpenAPI: SwarmCommon.yaml#/components/schemas/RttMs
/// Endpoint: POST /pingpong/{address}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingpongResponse {
    /// Round-trip time as duration string (nanoseconds)
    pub rtt: Duration,
}

/// Node health status
/// OpenAPI: SwarmCommon.yaml#/components/schemas/HealthStatus
/// Endpoint: GET /health
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    /// Health status: "ok", "nok", or "unknown"
    pub status: String,
    /// Bee version
    #[serde(default)]
    pub version: Option<String>,
    /// API version
    #[serde(default)]
    pub api_version: Option<String>,
}

/// Reachability status
/// OpenAPI: SwarmCommon.yaml - BzzTopology.reachability enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Reachability {
    Unknown,
    Public,
    Private,
}

/// Network availability status
/// OpenAPI: SwarmCommon.yaml - BzzTopology.networkAvailability enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NetworkAvailability {
    Unknown,
    Available,
    Unavailable,
}

/// Kademlia bin information
/// OpenAPI: SwarmCommon.yaml - BzzTopology.bins additionalProperties
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KademliaBin {
    pub population: i64,
    pub connected: i64,
    #[serde(default)]
    pub disconnected_peers: Option<Vec<PeerInfo>>,
    #[serde(default)]
    pub connected_peers: Option<Vec<PeerInfo>>,
}

/// Peer information in topology
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub address: SwarmAddress,
    #[serde(default)]
    pub full_node: Option<bool>,
}

/// Network topology information
/// OpenAPI: SwarmCommon.yaml#/components/schemas/BzzTopology
/// Endpoint: GET /topology
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Topology {
    /// Base overlay address
    pub base_addr: SwarmAddress,
    /// Total known peers
    pub population: i64,
    /// Currently connected peers
    pub connected: i64,
    /// Timestamp of topology snapshot
    #[serde(default)]
    pub timestamp: Option<String>,
    /// Nearest neighbor low watermark
    #[serde(default)]
    pub nn_low_watermark: Option<i64>,
    /// Kademlia depth
    pub depth: i64,
    /// Node reachability status
    #[serde(default)]
    pub reachability: Option<Reachability>,
    /// Network availability status
    #[serde(default)]
    pub network_availability: Option<NetworkAvailability>,
    /// Kademlia bins (bin index -> bin info)
    #[serde(default)]
    pub bins: std::collections::HashMap<String, KademliaBin>,
}

/// Generic API response
/// OpenAPI: SwarmCommon.yaml#/components/schemas/Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    pub message: String,
    #[serde(default)]
    pub code: Option<i64>,
}

/// API error response
/// OpenAPI: SwarmCommon.yaml#/components/schemas/ProblemDetails
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub code: Option<i64>,
}

/// Node mode
/// OpenAPI: SwarmCommon.yaml#/components/schemas/Node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BeeMode {
    Light,
    Full,
    Dev,
    UltraLight,
    Unknown,
}

/// Node information
/// OpenAPI: SwarmCommon.yaml#/components/schemas/Node
/// Endpoint: GET /node (part of readiness/status)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub bee_mode: BeeMode,
    #[serde(default)]
    pub chequebook_enabled: Option<bool>,
    #[serde(default)]
    pub swap_enabled: Option<bool>,
}

/// Readiness response
/// Endpoint: GET /readiness
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Readiness {
    pub status: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub api_version: Option<String>,
}
