//! Check trait and supporting types
//!
//! The `Check` trait defines the interface for all cluster checks.
//! Each check can be configured via `CheckOptions` and returns a `CheckResult`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

use crate::client::BeeClient;

/// Errors that can occur during check execution
#[derive(Debug, Error)]
pub enum CheckError {
    #[error("Client error: {0}")]
    Client(#[from] crate::client::BeeError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    #[error("Check failed: {0}")]
    Failed(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result of a single node's participation in a check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResult {
    /// Node name or identifier
    pub node: String,
    /// Whether this node passed
    pub passed: bool,
    /// Optional error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Additional details (check-specific)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub details: HashMap<String, serde_json::Value>,
}

impl NodeResult {
    /// Create a passing result for a node
    pub fn passed(node: impl Into<String>) -> Self {
        Self {
            node: node.into(),
            passed: true,
            error: None,
            details: HashMap::new(),
        }
    }

    /// Create a failing result for a node
    pub fn failed(node: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            node: node.into(),
            passed: false,
            error: Some(error.into()),
            details: HashMap::new(),
        }
    }

    /// Add a detail to the result
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(v) = serde_json::to_value(value) {
            self.details.insert(key.into(), v);
        }
        self
    }
}

/// Overall result of a check execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// Name of the check
    pub check_name: String,
    /// Whether the check passed overall
    pub passed: bool,
    /// Individual node results
    pub node_results: Vec<NodeResult>,
    /// How long the check took
    pub duration: Duration,
    /// Summary message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl CheckResult {
    /// Create a new check result
    pub fn new(
        check_name: impl Into<String>,
        node_results: Vec<NodeResult>,
        duration: Duration,
    ) -> Self {
        let passed = node_results.iter().all(|r| r.passed);
        Self {
            check_name: check_name.into(),
            passed,
            node_results,
            duration,
            message: None,
        }
    }

    /// Add a summary message
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

/// Configuration options for a check
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckOptions {
    /// Maximum time to wait for check completion
    #[serde(
        default,
        with = "humantime_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub timeout: Option<Duration>,

    /// Number of retry attempts for transient failures
    #[serde(default)]
    pub retries: u32,

    /// Delay between retry attempts
    #[serde(
        default,
        with = "humantime_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub retry_delay: Option<Duration>,

    /// Check-specific options (arbitrary key-value pairs)
    #[serde(default, flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl CheckOptions {
    /// Get the timeout or a default value
    pub fn timeout_or(&self, default: Duration) -> Duration {
        self.timeout.unwrap_or(default)
    }

    /// Get the retry delay or a default value
    pub fn retry_delay_or(&self, default: Duration) -> Duration {
        self.retry_delay.unwrap_or(default)
    }

    /// Get an extra option as a specific type
    pub fn get_extra<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.extra
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}

/// Context provided to checks during execution
#[derive(Clone)]
pub struct CheckContext {
    /// All nodes in the cluster (including bootnode)
    pub nodes: Vec<Arc<BeeClient>>,
    /// The bootnode (also included in nodes)
    pub bootnode: Arc<BeeClient>,
}

impl CheckContext {
    /// Create a new check context
    pub fn new(bootnode: BeeClient, nodes: Vec<BeeClient>) -> Self {
        let bootnode = Arc::new(bootnode);
        let mut all_nodes: Vec<Arc<BeeClient>> = vec![bootnode.clone()];
        all_nodes.extend(nodes.into_iter().map(Arc::new));

        Self {
            nodes: all_nodes,
            bootnode,
        }
    }

    /// Get all nodes except the bootnode
    pub fn non_bootnode_nodes(&self) -> impl Iterator<Item = &Arc<BeeClient>> {
        self.nodes.iter().skip(1)
    }

    /// Get the number of nodes (including bootnode)
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

/// Trait for implementing cluster checks
///
/// Each check verifies some aspect of cluster health or functionality.
/// Checks are registered in the `CHECKS` registry and can be invoked by name.
///
/// ## Example Implementation
///
/// ```ignore
/// use async_trait::async_trait;
/// use apiary::checks::{Check, CheckContext, CheckOptions, CheckResult, CheckError};
///
/// pub struct MyCheck;
///
/// #[async_trait]
/// impl Check for MyCheck {
///     fn name(&self) -> &'static str { "mycheck" }
///     fn description(&self) -> &'static str { "Verifies something important" }
///
///     async fn run(&self, ctx: &CheckContext, opts: &CheckOptions) -> Result<CheckResult, CheckError> {
///         // Implementation here
///         todo!()
///     }
/// }
/// ```
#[async_trait]
pub trait Check: Send + Sync {
    /// Unique name for this check (used in CLI and config)
    fn name(&self) -> &'static str;

    /// Human-readable description
    fn description(&self) -> &'static str;

    /// Run the check against the cluster
    async fn run(&self, ctx: &CheckContext, opts: &CheckOptions)
    -> Result<CheckResult, CheckError>;

    /// Default options for this check
    fn default_options(&self) -> CheckOptions {
        CheckOptions {
            timeout: Some(Duration::from_secs(300)), // 5 minutes default
            retries: 3,
            retry_delay: Some(Duration::from_secs(2)),
            extra: HashMap::new(),
        }
    }
}
