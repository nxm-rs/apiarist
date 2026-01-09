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

// =========================================================================
// Data Operation Types (P1 checks)
// =========================================================================

/// Swarm reference (64 hex characters for chunk/content address)
/// OpenAPI: SwarmCommon.yaml#/components/schemas/SwarmReference
pub type SwarmReference = String;

/// Batch ID (64 hex characters)
/// OpenAPI: SwarmCommon.yaml#/components/schemas/BatchID
pub type BatchId = String;

/// Tag UID (unsigned integer)
/// OpenAPI: SwarmCommon.yaml#/components/schemas/Uid
pub type TagUid = u64;

/// Reference response from upload operations
/// OpenAPI: SwarmCommon.yaml#/components/schemas/ReferenceResponse
/// Endpoints: POST /chunks, POST /bytes, POST /bzz
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceResponse {
    pub reference: SwarmReference,
}

/// Postage batch information
/// OpenAPI: SwarmCommon.yaml#/components/schemas/DebugPostageBatch
/// Endpoint: GET /stamps/{batch_id}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostageBatch {
    /// Batch ID
    #[serde(rename = "batchID")]
    pub batch_id: BatchId,
    /// Utilization percentage (0-100)
    pub utilization: u32,
    /// Whether the batch is usable
    pub usable: bool,
    /// Optional label
    #[serde(default)]
    pub label: Option<String>,
    /// Depth (log2 of max chunks)
    pub depth: u8,
    /// Amount per chunk
    #[serde(default)]
    pub amount: Option<String>,
    /// Bucket depth
    #[serde(rename = "bucketDepth")]
    pub bucket_depth: u8,
    /// Block number when batch was created
    #[serde(rename = "blockNumber")]
    pub block_number: u64,
    /// Whether batch is immutable
    #[serde(rename = "immutableFlag")]
    pub immutable: bool,
    /// Exists flag
    #[serde(default)]
    pub exists: Option<bool>,
    /// Batch TTL in blocks (if available)
    #[serde(default, rename = "batchTTL")]
    pub batch_ttl: Option<i64>,
}

/// List of postage batches
/// OpenAPI: SwarmCommon.yaml#/components/schemas/DebugPostageBatchesResponse
/// Endpoint: GET /stamps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostageBatches {
    pub stamps: Vec<PostageBatch>,
}

/// Response when creating a new postage batch
/// OpenAPI: SwarmCommon.yaml#/components/schemas/BatchIDResponse
/// Endpoint: POST /stamps/{amount}/{depth}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchIdResponse {
    /// The batch ID (note: API uses "batchID" not "batch_id")
    #[serde(rename = "batchID")]
    pub batch_id: BatchId,
    /// Transaction hash
    #[serde(default, rename = "txHash")]
    pub tx_hash: Option<String>,
}

/// Tag information for tracking uploads
/// OpenAPI: SwarmCommon.yaml#/components/schemas/NewTagResponse
/// Endpoints: POST /tags, GET /tags/{uid}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    /// Tag UID
    pub uid: TagUid,
    /// When the tag was started (ISO 8601)
    pub started_at: String,
    /// Chunks split during upload
    #[serde(default)]
    pub split: Option<u64>,
    /// Chunks seen
    #[serde(default)]
    pub seen: Option<u64>,
    /// Chunks stored locally
    #[serde(default)]
    pub stored: Option<u64>,
    /// Chunks sent to network
    #[serde(default)]
    pub sent: Option<u64>,
    /// Chunks synced to network
    #[serde(default)]
    pub synced: Option<u64>,
    /// Root address (if upload complete)
    #[serde(default)]
    pub address: Option<String>,
}

/// Wallet balance response
/// OpenAPI: SwarmCommon.yaml#/components/schemas/WalletResponse
/// Endpoint: GET /wallet
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletBalance {
    /// BZZ balance
    pub bzz_balance: String,
    /// Native token (xDAI) balance
    pub native_token_balance: String,
    /// Chain ID
    #[serde(default)]
    pub chain_id: Option<u64>,
    /// Chequebook contract address
    #[serde(default)]
    pub chequebook_contract_address: Option<String>,
    /// Wallet address
    #[serde(default)]
    pub wallet_address: Option<String>,
}

/// Chain state information
/// OpenAPI: SwarmCommon.yaml#/components/schemas/ChainState
/// Endpoint: GET /chainstate
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainState {
    /// Current chain tip block number
    #[serde(default)]
    pub chain_tip: Option<u64>,
    /// Current synced block number
    #[serde(default)]
    pub block: Option<u64>,
    /// Total amount of BZZ in postage stamps
    #[serde(default)]
    pub total_amount: Option<String>,
    /// Current price per chunk per block
    #[serde(default)]
    pub current_price: Option<String>,
}

/// Options for batch creation
#[derive(Debug, Clone)]
pub struct BatchOptions {
    /// Batch depth (log2 of max chunks, must be >= 17)
    pub depth: u8,
    /// Optional label for batch identification
    pub label: Option<String>,
    /// Target TTL in seconds (used to calculate amount if current_price available)
    /// Default: 24 hours
    pub ttl_secs: u64,
    /// Fallback amount if price calculation not possible
    /// Default: "100000000" (suitable for testing)
    pub fallback_amount: String,
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self {
            depth: 17,
            label: Some("apiarist".to_string()),
            ttl_secs: 24 * 60 * 60, // 24 hours
            fallback_amount: "100000000".to_string(),
        }
    }
}
