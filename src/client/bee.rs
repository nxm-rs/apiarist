//! Bee HTTP API Client
//!
//! Provides a typed client for interacting with Bee nodes.
//! Endpoints are based on the OpenAPI specification in `openapi/Swarm.yaml`.

use reqwest::Client;
use thiserror::Error;
use url::Url;

use super::types::*;
use crate::config::NodeType;
use crate::utils::HasId;

/// Errors that can occur when interacting with the Bee API
#[derive(Debug, Error)]
pub enum BeeError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("API error: {message} (code: {code:?})")]
    Api { message: String, code: Option<i64> },

    #[error("Unexpected response: {0}")]
    UnexpectedResponse(String),
}

/// Result type for Bee API operations
pub type BeeResult<T> = Result<T, BeeError>;

/// Client for interacting with a Bee node's HTTP API
///
/// # Example
/// ```no_run
/// use apiarist::client::BeeClient;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = BeeClient::new("http://localhost:1633")?;
/// let addresses = client.addresses().await?;
/// println!("Overlay: {}", addresses.overlay);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct BeeClient {
    /// Base URL of the Bee API (e.g., http://localhost:1633)
    base_url: Url,
    /// HTTP client for making requests
    client: Client,
    /// Node name for logging/identification
    name: Option<String>,
    /// Node type for connectivity expectations
    node_type: NodeType,
}

impl BeeClient {
    /// Create a new BeeClient for the given API URL
    ///
    /// # Arguments
    /// * `api_url` - Base URL of the Bee API (e.g., "http://localhost:1633")
    pub fn new(api_url: &str) -> BeeResult<Self> {
        let base_url = Url::parse(api_url)?;
        let client = Client::new();

        Ok(Self {
            base_url,
            client,
            name: None,
            node_type: NodeType::Full, // Default to full node
        })
    }

    /// Create a new BeeClient with a custom HTTP client
    pub fn with_client(api_url: &str, client: Client) -> BeeResult<Self> {
        let base_url = Url::parse(api_url)?;

        Ok(Self {
            base_url,
            client,
            name: None,
            node_type: NodeType::Full, // Default to full node
        })
    }

    /// Set a name for this client (useful for logging)
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the node type for this client
    pub fn with_node_type(mut self, node_type: NodeType) -> Self {
        self.node_type = node_type;
        self
    }

    /// Get the node name if set
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Get the node type
    pub fn node_type(&self) -> NodeType {
        self.node_type
    }

    /// Get the base URL
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    // =========================================================================
    // Connectivity Endpoints
    // OpenAPI: Swarm.yaml - /addresses, /peers, /pingpong, /topology
    // =========================================================================

    /// Get the node's overlay and underlay addresses
    ///
    /// OpenAPI: GET /addresses
    /// Returns: Addresses (overlay, underlay, ethereum, etc.)
    pub async fn addresses(&self) -> BeeResult<Addresses> {
        let url = self.base_url.join("addresses")?;
        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    /// Get list of connected peers
    ///
    /// OpenAPI: GET /peers
    /// Returns: Peers (list of peer addresses)
    pub async fn peers(&self) -> BeeResult<Peers> {
        let url = self.base_url.join("peers")?;
        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    /// Ping a peer and measure round-trip time
    ///
    /// OpenAPI: POST /pingpong/{address}
    /// Returns: PingpongResponse (RTT in nanoseconds)
    pub async fn pingpong(&self, peer_address: &str) -> BeeResult<PingpongResponse> {
        let url = self.base_url.join(&format!("pingpong/{peer_address}"))?;
        let response = self.client.post(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    /// Get the network topology
    ///
    /// OpenAPI: GET /topology
    /// Returns: Topology (Kademlia DHT information)
    pub async fn topology(&self) -> BeeResult<Topology> {
        let url = self.base_url.join("topology")?;
        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    // =========================================================================
    // Status Endpoints
    // OpenAPI: Swarm.yaml - /health, /readiness
    // =========================================================================

    /// Get node health status
    ///
    /// OpenAPI: GET /health
    /// Returns: HealthStatus (ok, nok, or unknown)
    pub async fn health(&self) -> BeeResult<HealthStatus> {
        let url = self.base_url.join("health")?;
        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    /// Check if node is ready
    ///
    /// OpenAPI: GET /readiness
    /// Returns: Readiness status
    pub async fn readiness(&self) -> BeeResult<Readiness> {
        let url = self.base_url.join("readiness")?;
        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    // =========================================================================
    // Chunk Endpoints
    // OpenAPI: Swarm.yaml - /chunks, /chunks/{address}
    // =========================================================================

    /// Upload a chunk to the network
    ///
    /// OpenAPI: POST /chunks
    /// Returns: ReferenceResponse with the chunk address
    ///
    /// # Arguments
    /// * `batch_id` - Postage batch ID to use for upload
    /// * `data` - Chunk data (span + payload, 8-4096 bytes)
    pub async fn upload_chunk(
        &self,
        batch_id: &str,
        data: impl Into<Vec<u8>>,
    ) -> BeeResult<ReferenceResponse> {
        let url = self.base_url.join("chunks")?;
        let response = self
            .client
            .post(url)
            .header("swarm-postage-batch-id", batch_id)
            .header("content-type", "application/octet-stream")
            .body(data.into())
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    /// Download a chunk from the network
    ///
    /// OpenAPI: GET /chunks/{address}
    /// Returns: Raw chunk data (span + payload)
    pub async fn download_chunk(&self, reference: &str) -> BeeResult<Vec<u8>> {
        let url = self.base_url.join(&format!("chunks/{reference}"))?;
        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.bytes().await?.to_vec())
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    /// Check if a chunk exists locally
    ///
    /// OpenAPI: HEAD /chunks/{address}
    /// Returns: true if chunk exists, false otherwise
    pub async fn has_chunk(&self, reference: &str) -> BeeResult<bool> {
        let url = self.base_url.join(&format!("chunks/{reference}"))?;
        let response = self.client.head(url).send().await?;

        Ok(response.status().is_success())
    }

    // =========================================================================
    // Bytes Endpoints
    // OpenAPI: Swarm.yaml - /bytes, /bytes/{reference}
    // =========================================================================

    /// Upload arbitrary data to the network
    ///
    /// OpenAPI: POST /bytes
    /// Returns: ReferenceResponse with the data address
    ///
    /// # Arguments
    /// * `batch_id` - Postage batch ID to use for upload
    /// * `data` - Arbitrary data to upload
    pub async fn upload_bytes(
        &self,
        batch_id: &str,
        data: impl Into<Vec<u8>>,
    ) -> BeeResult<ReferenceResponse> {
        let url = self.base_url.join("bytes")?;
        let response = self
            .client
            .post(url)
            .header("swarm-postage-batch-id", batch_id)
            .header("content-type", "application/octet-stream")
            .body(data.into())
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    /// Download data from the network
    ///
    /// OpenAPI: GET /bytes/{reference}
    /// Returns: Raw data bytes
    pub async fn download_bytes(&self, reference: &str) -> BeeResult<Vec<u8>> {
        let url = self.base_url.join(&format!("bytes/{reference}"))?;
        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.bytes().await?.to_vec())
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    // =========================================================================
    // Postage Stamp Endpoints
    // OpenAPI: Swarm.yaml - /stamps, /stamps/{batch_id}, /stamps/{amount}/{depth}
    // =========================================================================

    /// List all postage batches owned by this node
    ///
    /// OpenAPI: GET /stamps
    /// Returns: PostageBatches
    pub async fn list_stamps(&self) -> BeeResult<PostageBatches> {
        let url = self.base_url.join("stamps")?;
        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    /// Get a specific postage batch by ID
    ///
    /// OpenAPI: GET /stamps/{batch_id}
    /// Returns: PostageBatch
    pub async fn get_stamp(&self, batch_id: &str) -> BeeResult<PostageBatch> {
        let url = self.base_url.join(&format!("stamps/{batch_id}"))?;
        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    /// Buy a new postage batch
    ///
    /// OpenAPI: POST /stamps/{amount}/{depth}
    /// Returns: BatchIdResponse with the new batch ID
    ///
    /// # Arguments
    /// * `amount` - Amount of BZZ per chunk (as string for big integers)
    /// * `depth` - Batch depth (log2 of max chunks, must be > 16)
    /// * `label` - Optional label for the batch
    pub async fn create_stamp(
        &self,
        amount: &str,
        depth: u8,
        label: Option<&str>,
    ) -> BeeResult<BatchIdResponse> {
        let mut url = self.base_url.join(&format!("stamps/{amount}/{depth}"))?;

        if let Some(lbl) = label {
            url.query_pairs_mut().append_pair("label", lbl);
        }

        let response = self.client.post(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    // =========================================================================
    // Tag Endpoints
    // OpenAPI: Swarm.yaml - /tags, /tags/{uid}
    // =========================================================================

    /// Create a new upload tag for tracking
    ///
    /// OpenAPI: POST /tags
    /// Returns: Tag with the new UID
    pub async fn create_tag(&self) -> BeeResult<Tag> {
        let url = self.base_url.join("tags")?;
        let response = self.client.post(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    /// Get tag information by UID
    ///
    /// OpenAPI: GET /tags/{uid}
    /// Returns: Tag
    pub async fn get_tag(&self, uid: u64) -> BeeResult<Tag> {
        let url = self.base_url.join(&format!("tags/{uid}"))?;
        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }

    // =========================================================================
    // Wallet Endpoint
    // OpenAPI: Swarm.yaml - /wallet
    // =========================================================================

    /// Get wallet balance (BZZ and native token)
    ///
    /// OpenAPI: GET /wallet
    /// Returns: WalletBalance
    pub async fn wallet(&self) -> BeeResult<WalletBalance> {
        let url = self.base_url.join("wallet")?;
        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                message: Some("Unknown error".into()),
                code: None,
            });
            Err(BeeError::Api {
                message: error.message.unwrap_or_default(),
                code: error.code,
            })
        }
    }
}

// Implement HasId for BeeClient to enable use with concurrent utilities
impl HasId for BeeClient {
    fn id(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| self.base_url.to_string())
    }
}

// Note: Arc<BeeClient> gets HasId via the blanket impl<T: HasId> HasId for Arc<T>

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::HasId;

    #[test]
    fn test_client_creation() {
        let client = BeeClient::new("http://localhost:1633").unwrap();
        assert_eq!(client.base_url().as_str(), "http://localhost:1633/");
    }

    #[test]
    fn test_client_with_name() {
        let client = BeeClient::new("http://localhost:1633")
            .unwrap()
            .with_name("bootnode");
        assert_eq!(client.name(), Some("bootnode"));
    }

    #[test]
    fn test_client_default_node_type() {
        let client = BeeClient::new("http://localhost:1633").unwrap();
        assert_eq!(client.node_type(), NodeType::Full);
    }

    #[test]
    fn test_client_with_node_type() {
        let client = BeeClient::new("http://localhost:1633")
            .unwrap()
            .with_node_type(NodeType::Boot);
        assert_eq!(client.node_type(), NodeType::Boot);

        let client = BeeClient::new("http://localhost:1633")
            .unwrap()
            .with_node_type(NodeType::Light);
        assert_eq!(client.node_type(), NodeType::Light);
    }

    #[test]
    fn test_has_id_with_name() {
        let client = BeeClient::new("http://localhost:1633")
            .unwrap()
            .with_name("node-1");
        assert_eq!(client.id(), "node-1");
    }

    #[test]
    fn test_has_id_without_name() {
        let client = BeeClient::new("http://localhost:1633").unwrap();
        assert_eq!(client.id(), "http://localhost:1633/");
    }
}
