//! Smoke Check
//!
//! Verifies basic upload/download functionality across the swarm network.
//!
//! ## What it checks
//!
//! 1. Can upload random data to a node
//! 2. Can download the same data from a different node
//! 3. Downloaded data matches uploaded data
//!
//! ## Run Modes
//!
//! - **Single-shot** (default): Run once and exit
//! - **Long-running**: Run continuously for a specified duration (like beekeeper)
//!
//! ## Equivalent beekeeper check
//!
//! Reference: `beekeeper/pkg/check/smoke/smoke.go`
//!
//! ## Options
//!
//! - `timeout`: Maximum time for single operation (default: 5m)
//! - `duration_secs`: Total duration to run in seconds (default: 0 = single shot)
//! - `data_size`: Size of random data to upload in bytes (default: 1024)
//! - `file_sizes`: Array of file sizes to test per iteration (default: [1024])
//! - `seed`: Random seed for reproducible data (default: random)
//! - `batch_id`: Specific batch ID to use (default: auto-detect from node)
//! - `sync_wait_ms`: Milliseconds to wait after upload for network sync (default: 2000)
//! - `iteration_wait_secs`: Seconds to wait between iterations (default: 5)

use async_trait::async_trait;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

use super::traits::{Check, CheckContext, CheckError, CheckOptions, CheckResult, NodeResult};

/// Default data size for smoke test (1 KB)
const DEFAULT_DATA_SIZE: usize = 1024;

/// Default sync wait after upload before downloading (to allow network sync)
/// With batches pre-created and warmup disabled (BEE_WARMUP_TIME=0s), chunks
/// propagate quickly in local devnets. We use 5 seconds as a reasonable wait.
/// For slower networks, this can be increased via check options.
const DEFAULT_SYNC_WAIT_MS: u64 = 5_000;

/// Default wait between iterations in long-running mode (seconds)
const DEFAULT_ITERATION_WAIT_SECS: u64 = 5;

/// Number of retries for upload/download operations (like beekeeper's 3 retries)
const DEFAULT_RETRIES: usize = 3;

/// Wait time between retries in milliseconds (like beekeeper's TxOnErrWait/RxOnErrWait = 10s)
const DEFAULT_RETRY_WAIT_MS: u64 = 10_000;

/// Statistics for a single iteration
#[derive(Debug, Clone, Default)]
struct IterationStats {
    uploads_attempted: u64,
    uploads_succeeded: u64,
    downloads_attempted: u64,
    downloads_succeeded: u64,
    data_uploaded_bytes: u64,
    data_downloaded_bytes: u64,
    mismatches: u64,
}

impl IterationStats {
    fn merge(&mut self, other: &IterationStats) {
        self.uploads_attempted += other.uploads_attempted;
        self.uploads_succeeded += other.uploads_succeeded;
        self.downloads_attempted += other.downloads_attempted;
        self.downloads_succeeded += other.downloads_succeeded;
        self.data_uploaded_bytes += other.data_uploaded_bytes;
        self.data_downloaded_bytes += other.data_downloaded_bytes;
        self.mismatches += other.mismatches;
    }
}

/// Smoke check for basic upload/download functionality
///
/// This check verifies that:
/// 1. Data can be uploaded to the swarm via one node
/// 2. Data can be retrieved from a different node
/// 3. The retrieved data matches the original
pub struct SmokeCheck;

impl SmokeCheck {
    /// Generate random data of the specified size with the given seed state
    fn generate_data(size: usize, seed: &mut u64) -> Vec<u8> {
        let mut data = Vec::with_capacity(size);

        for _ in 0..size {
            // Simple xorshift-like PRNG
            *seed ^= *seed << 13;
            *seed ^= *seed >> 7;
            *seed ^= *seed << 17;
            data.push(*seed as u8);
        }

        data
    }

    /// Run a single upload/download iteration for one file size
    ///
    /// Like beekeeper, this uses exactly TWO full nodes: one uploader and one downloader.
    /// It does NOT download from all nodes - just the designated downloader.
    /// Includes retry logic matching beekeeper (3 retries with 10s wait).
    #[allow(clippy::too_many_arguments)]
    async fn run_iteration(
        upload_node: &crate::client::BeeClient,
        download_node: &crate::client::BeeClient,
        batch_id: &str,
        data_size: usize,
        seed: &mut u64,
        sync_wait_ms: u64,
        retries: usize,
        retry_wait_ms: u64,
        node_results: &mut Vec<NodeResult>,
    ) -> IterationStats {
        let mut stats = IterationStats::default();

        // Generate random data
        let data = Self::generate_data(data_size, seed);
        debug!(data_size = data.len(), "Generated random data");

        let upload_node_name = upload_node.name().unwrap_or("upload_node").to_string();
        let download_node_name = download_node.name().unwrap_or("download_node").to_string();

        // Upload data with retries (like beekeeper's upload retry loop)
        stats.uploads_attempted += 1;
        let mut reference: Option<String> = None;
        let mut last_upload_error: Option<String> = None;

        for attempt in 0..retries {
            if attempt > 0 {
                info!(
                    node = %upload_node_name,
                    attempt = attempt + 1,
                    retries = retries,
                    wait_ms = retry_wait_ms,
                    "Retrying upload"
                );
                tokio::time::sleep(Duration::from_millis(retry_wait_ms)).await;
            }

            info!(node = %upload_node_name, data_size = data.len(), attempt = attempt + 1, "Uploading data");

            match upload_node.upload_bytes(batch_id, data.clone()).await {
                Ok(resp) => {
                    stats.uploads_succeeded += 1;
                    stats.data_uploaded_bytes += data_size as u64;
                    info!(
                        node = %upload_node_name,
                        reference = %resp.reference,
                        attempt = attempt + 1,
                        "Upload successful"
                    );
                    node_results.push(
                        NodeResult::passed(&upload_node_name)
                            .with_detail("action", "upload")
                            .with_detail("reference", &resp.reference)
                            .with_detail("data_size", data_size)
                            .with_detail("attempts", attempt + 1),
                    );
                    reference = Some(resp.reference);
                    break;
                }
                Err(e) => {
                    warn!(node = %upload_node_name, error = %e, attempt = attempt + 1, "Upload failed");
                    last_upload_error = Some(format!("{e}"));
                }
            }
        }

        let reference = match reference {
            Some(r) => r,
            None => {
                let err_msg = last_upload_error.unwrap_or_else(|| "Unknown error".to_string());
                error!(node = %upload_node_name, error = %err_msg, retries = retries, "Upload failed after all retries");
                node_results.push(
                    NodeResult::failed(
                        &upload_node_name,
                        format!("Upload failed after {retries} attempts: {err_msg}"),
                    )
                    .with_detail("action", "upload")
                    .with_detail("data_size", data_size),
                );
                return stats;
            }
        };

        // Wait for network sync before downloading
        if sync_wait_ms > 0 {
            debug!(sync_wait_ms = sync_wait_ms, "Waiting for network sync");
            tokio::time::sleep(Duration::from_millis(sync_wait_ms)).await;
        }

        // Download from the designated download node with retries (like beekeeper's download retry loop)
        stats.downloads_attempted += 1;
        let mut download_success = false;
        let mut last_download_error: Option<String> = None;

        for attempt in 0..retries {
            if attempt > 0 {
                info!(
                    node = %download_node_name,
                    attempt = attempt + 1,
                    retries = retries,
                    wait_ms = retry_wait_ms,
                    "Retrying download"
                );
                tokio::time::sleep(Duration::from_millis(retry_wait_ms)).await;
            }

            debug!(node = %download_node_name, reference = %reference, attempt = attempt + 1, "Downloading data");

            match download_node.download_bytes(&reference).await {
                Ok(downloaded) => {
                    if downloaded == data {
                        stats.downloads_succeeded += 1;
                        stats.data_downloaded_bytes += downloaded.len() as u64;
                        info!(node = %download_node_name, attempt = attempt + 1, "Download successful, data matches");
                        node_results.push(
                            NodeResult::passed(&download_node_name)
                                .with_detail("action", "download")
                                .with_detail("reference", &reference)
                                .with_detail("data_size", downloaded.len())
                                .with_detail("attempts", attempt + 1),
                        );
                        download_success = true;
                        break;
                    } else {
                        stats.mismatches += 1;
                        warn!(
                            node = %download_node_name,
                            expected_size = data.len(),
                            actual_size = downloaded.len(),
                            attempt = attempt + 1,
                            "Data mismatch"
                        );
                        last_download_error = Some(format!(
                            "Data mismatch: expected {} bytes, got {}",
                            data.len(),
                            downloaded.len()
                        ));
                        // Data mismatch is not retryable
                        node_results.push(
                            NodeResult::failed(
                                &download_node_name,
                                format!(
                                    "Data mismatch: expected {} bytes, got {}",
                                    data.len(),
                                    downloaded.len()
                                ),
                            )
                            .with_detail("action", "download")
                            .with_detail("expected_size", data.len())
                            .with_detail("actual_size", downloaded.len()),
                        );
                        break;
                    }
                }
                Err(e) => {
                    warn!(node = %download_node_name, error = %e, attempt = attempt + 1, "Download failed");
                    last_download_error = Some(format!("{e}"));
                }
            }
        }

        if !download_success && stats.mismatches == 0 {
            let err_msg = last_download_error.unwrap_or_else(|| "Unknown error".to_string());
            error!(node = %download_node_name, error = %err_msg, retries = retries, "Download failed after all retries");
            node_results.push(
                NodeResult::failed(
                    &download_node_name,
                    format!("Download failed after {retries} attempts: {err_msg}"),
                )
                .with_detail("action", "download")
                .with_detail("reference", &reference),
            );
        }

        stats
    }
}

#[async_trait]
impl Check for SmokeCheck {
    fn name(&self) -> &'static str {
        "smoke"
    }

    fn description(&self) -> &'static str {
        "Upload/download sanity check (supports long-running mode)"
    }

    async fn run(
        &self,
        ctx: &CheckContext,
        opts: &CheckOptions,
    ) -> Result<CheckResult, CheckError> {
        let start = Instant::now();

        // Get options
        let duration_secs: u64 = opts.get_extra("duration_secs").unwrap_or(0);
        let file_sizes: Vec<usize> = opts
            .get_extra("file_sizes")
            .unwrap_or_else(|| vec![opts.get_extra("data_size").unwrap_or(DEFAULT_DATA_SIZE)]);
        let seed: u64 = opts.get_extra("seed").unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(42)
        });
        let specified_batch: Option<String> = opts.get_extra("batch_id");
        let sync_wait_ms: u64 = opts
            .get_extra("sync_wait_ms")
            .unwrap_or(DEFAULT_SYNC_WAIT_MS);
        let iteration_wait_secs: u64 = opts
            .get_extra("iteration_wait_secs")
            .unwrap_or(DEFAULT_ITERATION_WAIT_SECS);

        let is_long_running = duration_secs > 0;

        // Like beekeeper's smoke check, we need exactly 2 full nodes (not bootnodes)
        // Beekeeper uses ShuffledFullNodeClients and picks [0] as uploader, [1] as downloader
        let full_nodes = ctx.full_nodes();
        if full_nodes.len() < 2 {
            return Err(CheckError::Config(
                "Smoke check requires at least 2 full nodes (not bootnodes)".to_string(),
            ));
        }

        // Use first full node as uploader, second as downloader (like beekeeper)
        let upload_node = &full_nodes[0];
        let download_node = &full_nodes[1];
        let upload_node_name = upload_node.name().unwrap_or("unknown");
        let download_node_name = download_node.name().unwrap_or("unknown");

        info!(
            full_nodes = full_nodes.len(),
            upload_node = %upload_node_name,
            download_node = %download_node_name,
            file_sizes = ?file_sizes,
            seed = seed,
            duration_secs = duration_secs,
            long_running = is_long_running,
            "Starting smoke check"
        );

        // Get or create batch on the upload node
        let batch_id = if let Some(bid) = specified_batch {
            bid
        } else {
            let batch_opts = crate::client::BatchOptions::default();
            match upload_node.get_or_create_batch(&batch_opts).await {
                Ok(bid) => {
                    debug!(node = %upload_node_name, batch_id = %bid, "Got batch for smoke check");
                    bid
                }
                Err(e) => {
                    return Err(CheckError::Config(format!(
                        "Failed to get/create batch on {}: {}",
                        upload_node_name, e
                    )));
                }
            }
        };

        debug!(batch_id = %batch_id, from_node = %upload_node_name, "Using batch");

        let mut all_node_results = Vec::new();
        let mut total_stats = IterationStats::default();
        let mut iteration_count = 0u64;
        let mut current_seed = seed;

        let end_time = if is_long_running {
            Some(Instant::now() + Duration::from_secs(duration_secs))
        } else {
            None
        };

        loop {
            iteration_count += 1;
            info!(iteration = iteration_count, "Starting iteration");

            // Test each file size in this iteration
            for &file_size in &file_sizes {
                let mut iter_results = Vec::new();
                let stats = Self::run_iteration(
                    upload_node.as_ref(),
                    download_node.as_ref(),
                    &batch_id,
                    file_size,
                    &mut current_seed,
                    sync_wait_ms,
                    DEFAULT_RETRIES,
                    DEFAULT_RETRY_WAIT_MS,
                    &mut iter_results,
                )
                .await;

                total_stats.merge(&stats);

                // In long-running mode, we don't accumulate all node results to avoid memory bloat
                // Instead we only keep the last iteration's results
                if is_long_running {
                    all_node_results = iter_results;
                } else {
                    all_node_results.extend(iter_results);
                }

                info!(
                    iteration = iteration_count,
                    file_size = file_size,
                    uploads = format!("{}/{}", stats.uploads_succeeded, stats.uploads_attempted),
                    downloads = format!(
                        "{}/{}",
                        stats.downloads_succeeded, stats.downloads_attempted
                    ),
                    "File size test complete"
                );
            }

            // Check if we should continue (long-running mode)
            if let Some(end) = end_time {
                if Instant::now() >= end {
                    info!(iterations = iteration_count, "Duration reached, stopping");
                    break;
                }

                // Wait between iterations
                if iteration_wait_secs > 0 {
                    debug!(
                        wait_secs = iteration_wait_secs,
                        "Waiting before next iteration"
                    );
                    tokio::time::sleep(Duration::from_secs(iteration_wait_secs)).await;
                }
            } else {
                // Single-shot mode, exit after first iteration
                break;
            }
        }

        let duration = start.elapsed();
        let passed = total_stats.uploads_succeeded > 0
            && total_stats.downloads_succeeded > 0
            && total_stats.mismatches == 0
            && all_node_results.iter().all(|r| r.passed);

        info!(
            passed = passed,
            iterations = iteration_count,
            uploads = format!(
                "{}/{}",
                total_stats.uploads_succeeded, total_stats.uploads_attempted
            ),
            downloads = format!(
                "{}/{}",
                total_stats.downloads_succeeded, total_stats.downloads_attempted
            ),
            mismatches = total_stats.mismatches,
            data_uploaded_mb = total_stats.data_uploaded_bytes as f64 / 1_000_000.0,
            data_downloaded_mb = total_stats.data_downloaded_bytes as f64 / 1_000_000.0,
            duration_secs = duration.as_secs(),
            "Smoke check complete"
        );

        let message = if is_long_running {
            format!(
                "{} iterations, uploaded {:.2} MB, downloaded {:.2} MB, {} mismatches",
                iteration_count,
                total_stats.data_uploaded_bytes as f64 / 1_000_000.0,
                total_stats.data_downloaded_bytes as f64 / 1_000_000.0,
                total_stats.mismatches
            )
        } else {
            format!(
                "Uploaded {} bytes, downloaded successfully from {}/{} nodes",
                file_sizes.iter().sum::<usize>(),
                total_stats.downloads_succeeded,
                total_stats.downloads_attempted
            )
        };

        Ok(CheckResult::new("smoke", all_node_results, duration).with_message(message))
    }

    fn default_options(&self) -> CheckOptions {
        CheckOptions {
            timeout: Some(Duration::from_secs(300)), // 5 minutes per operation
            retries: 2,
            retry_delay: Some(Duration::from_secs(5)),
            extra: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_metadata() {
        let check = SmokeCheck;
        assert_eq!(check.name(), "smoke");
        assert!(!check.description().is_empty());
    }

    #[test]
    fn test_default_options() {
        let check = SmokeCheck;
        let opts = check.default_options();
        assert_eq!(opts.timeout, Some(Duration::from_secs(300)));
        assert_eq!(opts.retries, 2);
    }

    #[test]
    fn test_generate_data_deterministic() {
        let mut seed1 = 12345u64;
        let mut seed2 = 12345u64;
        let data1 = SmokeCheck::generate_data(100, &mut seed1);
        let data2 = SmokeCheck::generate_data(100, &mut seed2);
        assert_eq!(data1, data2);
    }

    #[test]
    fn test_generate_data_different_seeds() {
        let mut seed1 = 12345u64;
        let mut seed2 = 54321u64;
        let data1 = SmokeCheck::generate_data(100, &mut seed1);
        let data2 = SmokeCheck::generate_data(100, &mut seed2);
        assert_ne!(data1, data2);
    }

    #[test]
    fn test_generate_data_correct_size() {
        let mut seed = 42u64;
        let data = SmokeCheck::generate_data(1024, &mut seed);
        assert_eq!(data.len(), 1024);
    }

    #[test]
    fn test_iteration_stats_merge() {
        let mut stats1 = IterationStats {
            uploads_attempted: 1,
            uploads_succeeded: 1,
            downloads_attempted: 3,
            downloads_succeeded: 3,
            data_uploaded_bytes: 1000,
            data_downloaded_bytes: 3000,
            mismatches: 0,
        };

        let stats2 = IterationStats {
            uploads_attempted: 1,
            uploads_succeeded: 1,
            downloads_attempted: 3,
            downloads_succeeded: 2,
            data_uploaded_bytes: 2000,
            data_downloaded_bytes: 4000,
            mismatches: 1,
        };

        stats1.merge(&stats2);

        assert_eq!(stats1.uploads_attempted, 2);
        assert_eq!(stats1.uploads_succeeded, 2);
        assert_eq!(stats1.downloads_attempted, 6);
        assert_eq!(stats1.downloads_succeeded, 5);
        assert_eq!(stats1.data_uploaded_bytes, 3000);
        assert_eq!(stats1.data_downloaded_bytes, 7000);
        assert_eq!(stats1.mismatches, 1);
    }
}
