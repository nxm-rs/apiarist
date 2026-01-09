//! Single Owner Chunk (SOC) Check
//!
//! Verifies that single owner chunks can be uploaded and retrieved correctly.
//!
//! ## What it checks
//!
//! 1. Generate a secp256k1 private key
//! 2. Create a Content Addressed Chunk (CAC) with test payload
//! 3. Create a SOC by signing the ID and CAC address
//! 4. Upload the SOC to a node
//! 5. Download and verify the chunk data matches
//!
//! ## Equivalent beekeeper check
//!
//! Reference: `beekeeper/pkg/check/soc/soc.go`
//!
//! ## Options
//!
//! - `timeout`: Maximum time for the operation (default: 5m)
//! - `batch_id`: Specific batch ID to use (default: auto-create)

use alloy_primitives::B256;
use alloy_signer_local::PrivateKeySigner;
use async_trait::async_trait;
use bytes::Bytes;
use nectar_primitives::Chunk;
use nectar_primitives::chunk::{ContentChunk, SingleOwnerChunk};
use rand::Rng;
use std::time::Instant;
use tracing::{debug, info};

use super::traits::{Check, CheckContext, CheckError, CheckOptions, CheckResult, NodeResult};
use crate::client::BatchOptions;

/// The payload used in the SOC test (matches beekeeper)
const SOC_PAYLOAD: &[u8] = b"Hello Swarm :)";

/// Single Owner Chunk check
///
/// This check verifies that SOCs can be:
/// 1. Created and signed correctly
/// 2. Uploaded to a node
/// 3. Retrieved and verified
pub struct SocCheck;

impl SocCheck {
    /// Generate a random 32-byte ID
    fn random_id() -> B256 {
        let mut rng = rand::rng();
        let bytes: [u8; 32] = rng.random();
        B256::from(bytes)
    }

    /// Run the SOC test on a single node
    async fn run_soc_test(
        _ctx: &CheckContext,
        node: &crate::client::BeeClient,
        batch_id: &str,
    ) -> Result<NodeResult, CheckError> {
        let node_name = node.name().unwrap_or("unknown").to_string();

        info!(node = %node_name, "Running SOC test");

        // Step 1: Generate a random secp256k1 private key
        let signer = PrivateKeySigner::random();
        debug!(node = %node_name, "Generated random signer");

        // Step 2: Create a Content Addressed Chunk (CAC) with the test payload
        let cac = ContentChunk::new(Bytes::from_static(SOC_PAYLOAD))
            .map_err(|e| CheckError::Internal(format!("Failed to create CAC: {}", e)))?;

        debug!(
            node = %node_name,
            cac_address = %hex::encode(cac.address().as_bytes()),
            "Created CAC"
        );

        // Step 3: Generate a random ID
        let id = Self::random_id();
        debug!(node = %node_name, id = %hex::encode(id), "Generated random ID");

        // Step 4: Create and sign the SOC
        // SOC address = keccak256(id || owner)
        // Signature signs keccak256(id || cac.address)
        let soc = SingleOwnerChunk::new(id, Bytes::from_static(SOC_PAYLOAD), &signer)
            .map_err(|e| CheckError::Internal(format!("Failed to create SOC: {}", e)))?;

        let owner = signer.address();
        let signature = soc.signature();
        let soc_address_hex = hex::encode(soc.address().as_bytes());

        // Extract values before moving soc
        let owner_hex = hex::encode(owner.as_slice());
        let id_hex = hex::encode(id);
        let sig_hex = hex::encode(signature.as_bytes());

        info!(
            node = %node_name,
            soc_address = %soc_address_hex,
            owner = %owner_hex,
            id = %id_hex,
            "Created SOC"
        );

        // Step 5: Upload the SOC
        // The API expects: POST /soc/{owner}/{id}?sig={signature}
        // Body: CAC data (span + payload)

        // Get the CAC data (span + payload) to send as body
        let cac_data: Bytes = cac.into();

        debug!(
            node = %node_name,
            owner = %owner_hex,
            id = %id_hex,
            sig = %sig_hex,
            cac_data_len = cac_data.len(),
            "Uploading SOC"
        );

        let upload_result = node
            .upload_soc(&owner_hex, &id_hex, &sig_hex, cac_data.to_vec(), batch_id)
            .await;

        let reference = match upload_result {
            Ok(resp) => {
                info!(
                    node = %node_name,
                    reference = %resp.reference,
                    "SOC uploaded successfully"
                );
                resp.reference
            }
            Err(e) => {
                return Ok(NodeResult::failed(
                    &node_name,
                    format!("Failed to upload SOC: {}", e),
                ));
            }
        };

        // Step 6: Download the chunk and verify
        // The downloaded data should be the full SOC: id + signature + cac_data
        let downloaded = match node.download_chunk(&reference).await {
            Ok(data) => data,
            Err(e) => {
                return Ok(NodeResult::failed(
                    &node_name,
                    format!("Failed to download SOC: {}", e),
                ));
            }
        };

        info!(
            node = %node_name,
            downloaded_len = downloaded.len(),
            "Downloaded SOC chunk"
        );

        // Step 7: Verify the downloaded data matches the original SOC data
        // SOC data format: id (32 bytes) + signature (65 bytes) + cac_data (span + payload)
        let expected_soc_data: Bytes = soc.into();

        if downloaded != expected_soc_data.as_ref() {
            return Ok(NodeResult::failed(
                &node_name,
                format!(
                    "Downloaded SOC data does not match: expected {} bytes, got {} bytes",
                    expected_soc_data.len(),
                    downloaded.len()
                ),
            ));
        }

        info!(node = %node_name, "SOC verification passed");

        Ok(NodeResult::passed(&node_name)
            .with_detail("soc_address", soc_address_hex)
            .with_detail("owner", owner_hex)
            .with_detail("id", id_hex))
    }
}

#[async_trait]
impl Check for SocCheck {
    fn name(&self) -> &'static str {
        "soc"
    }

    fn description(&self) -> &'static str {
        "Verifies single owner chunk upload and retrieval"
    }

    async fn run(
        &self,
        ctx: &CheckContext,
        opts: &CheckOptions,
    ) -> Result<CheckResult, CheckError> {
        let start = Instant::now();
        info!("Starting SOC check");

        // Get full nodes (SOC requires full node capabilities)
        let full_nodes = ctx.full_capable_nodes();
        if full_nodes.is_empty() {
            return Err(CheckError::Config(
                "SOC test requires at least 1 full node".to_string(),
            ));
        }

        // Select a random node (like beekeeper does with ShuffledFullNodeClients)
        let node_idx = rand::rng().random_range(0..full_nodes.len());
        let node = full_nodes[node_idx];
        let node_name = node.name().unwrap_or("unknown").to_string();

        info!(node = %node_name, "Selected node for SOC test");

        // Get or create a postage batch
        let batch_id = if let Some(id) = opts.get_extra::<String>("batch_id") {
            info!(batch_id = %id, "Using provided batch ID");
            id
        } else {
            let batch_opts = BatchOptions::default();
            node.get_or_create_batch(&batch_opts)
                .await
                .map_err(CheckError::Client)?
        };

        info!(batch_id = %batch_id, "Using batch for SOC upload");

        // Run the SOC test
        let result = Self::run_soc_test(ctx, node, &batch_id).await?;
        let passed = result.passed;

        let duration = start.elapsed();
        let node_results = vec![result];

        let check_result =
            CheckResult::new(self.name(), node_results, duration).with_message(if passed {
                "SOC upload and retrieval successful".to_string()
            } else {
                "SOC test failed".to_string()
            });

        if passed {
            info!(duration = ?duration, "SOC check passed");
        } else {
            tracing::error!(duration = ?duration, "SOC check failed");
        }

        Ok(check_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_id_generates_32_bytes() {
        let id = SocCheck::random_id();
        assert_eq!(id.len(), 32);
    }

    #[test]
    fn test_random_id_is_random() {
        let id1 = SocCheck::random_id();
        let id2 = SocCheck::random_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_soc_payload_matches_beekeeper() {
        assert_eq!(SOC_PAYLOAD, b"Hello Swarm :)");
    }
}
