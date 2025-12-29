//! Cluster configuration types
//!
//! Defines the structure for cluster configuration files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

use crate::checks::{CheckContext, CheckOptions};
use crate::client::BeeClient;

/// Type of node in the Swarm network
///
/// Different node types have different connectivity expectations:
/// - Boot: Must connect to all other nodes
/// - Full: Must connect to all other full nodes
/// - Light: Must have at least 1 peer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    /// Bootnode - connected to all nodes
    Boot,
    /// Full node - connected to all full nodes
    #[default]
    Full,
    /// Light node - connected to at least one node
    Light,
}

impl NodeType {
    /// Check if this is a boot node
    pub fn is_boot(&self) -> bool {
        matches!(self, NodeType::Boot)
    }

    /// Check if this is a full node (includes boot nodes for connectivity purposes)
    pub fn is_full(&self) -> bool {
        matches!(self, NodeType::Boot | NodeType::Full)
    }

    /// Check if this is a light node
    pub fn is_light(&self) -> bool {
        matches!(self, NodeType::Light)
    }
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::Boot => write!(f, "boot"),
            NodeType::Full => write!(f, "full"),
            NodeType::Light => write!(f, "light"),
        }
    }
}

/// Errors that can occur during configuration
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse config: {0}")]
    Parse(#[from] serde_yaml::Error),

    #[error("Invalid configuration: {0}")]
    Invalid(String),

    #[error("Failed to create client: {0}")]
    Client(#[from] crate::client::BeeError),
}

/// Configuration for a single node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Node identifier
    pub name: String,
    /// HTTP API URL (e.g., "http://localhost:1633")
    pub api_url: String,
    /// Node type (boot, full, light) - defaults to Full for backwards compatibility
    #[serde(default)]
    pub node_type: NodeType,
}

/// Cluster configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Cluster name
    #[serde(default = "default_cluster_name")]
    pub name: String,

    /// Bootnode configuration
    pub bootnode: NodeConfig,

    /// Other nodes in the cluster
    #[serde(default)]
    pub nodes: Vec<NodeConfig>,
}

fn default_cluster_name() -> String {
    "default".to_string()
}

impl ClusterConfig {
    /// Create a CheckContext from this cluster configuration
    pub fn to_check_context(&self) -> Result<CheckContext, ConfigError> {
        let bootnode = BeeClient::new(&self.bootnode.api_url)?
            .with_name(&self.bootnode.name)
            .with_node_type(self.bootnode.node_type);

        let nodes: Result<Vec<BeeClient>, _> = self
            .nodes
            .iter()
            .map(|n| {
                BeeClient::new(&n.api_url).map(|c| c.with_name(&n.name).with_node_type(n.node_type))
            })
            .collect();

        Ok(CheckContext::new(bootnode, nodes?))
    }
}

/// Configuration for a single check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckConfig {
    /// Whether this check is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Timeout for this check
    #[serde(
        default,
        with = "humantime_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub timeout: Option<Duration>,

    /// Number of retries
    #[serde(default)]
    pub retries: Option<u32>,

    /// Delay between retries
    #[serde(
        default,
        with = "humantime_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub retry_delay: Option<Duration>,

    /// Additional check-specific options
    #[serde(default, flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_enabled() -> bool {
    true
}

impl CheckConfig {
    /// Convert to CheckOptions
    pub fn to_check_options(&self, defaults: &CheckOptions) -> CheckOptions {
        CheckOptions {
            timeout: self.timeout.or(defaults.timeout),
            retries: self.retries.unwrap_or(defaults.retries),
            retry_delay: self.retry_delay.or(defaults.retry_delay),
            extra: self.extra.clone(),
        }
    }
}

impl Default for CheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout: None,
            retries: None,
            retry_delay: None,
            extra: HashMap::new(),
        }
    }
}

/// Top-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Cluster configuration
    pub cluster: ClusterConfig,

    /// Check configurations (check_name -> config)
    #[serde(default)]
    pub checks: HashMap<String, CheckConfig>,
}

impl Config {
    /// Load configuration from a YAML file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        Self::from_yaml(&content)
    }

    /// Parse configuration from YAML string
    pub fn from_yaml(yaml: &str) -> Result<Self, ConfigError> {
        Ok(serde_yaml::from_str(yaml)?)
    }

    /// Get configuration for a specific check
    pub fn check_config(&self, name: &str) -> Option<&CheckConfig> {
        self.checks.get(name)
    }

    /// Check if a specific check is enabled
    pub fn is_check_enabled(&self, name: &str) -> bool {
        self.checks.get(name).map(|c| c.enabled).unwrap_or(true) // Default to enabled if not specified
    }

    /// Get list of enabled checks
    pub fn enabled_checks(&self) -> Vec<&str> {
        self.checks
            .iter()
            .filter(|(_, c)| c.enabled)
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Generate a default configuration
    pub fn default_config() -> Self {
        Config {
            cluster: ClusterConfig {
                name: "my-cluster".to_string(),
                bootnode: NodeConfig {
                    name: "bootnode".to_string(),
                    api_url: "http://bootnode:1633".to_string(),
                    node_type: NodeType::Boot,
                },
                nodes: vec![
                    NodeConfig {
                        name: "bee-0".to_string(),
                        api_url: "http://bee-0:1633".to_string(),
                        node_type: NodeType::Full,
                    },
                    NodeConfig {
                        name: "bee-1".to_string(),
                        api_url: "http://bee-1:1633".to_string(),
                        node_type: NodeType::Full,
                    },
                ],
            },
            checks: {
                let mut checks = HashMap::new();
                checks.insert("pingpong".to_string(), CheckConfig::default());
                checks.insert("peercount".to_string(), CheckConfig::default());
                checks.insert("fullconnectivity".to_string(), CheckConfig::default());
                checks
            },
        }
    }

    /// Serialize to YAML string
    pub fn to_yaml(&self) -> Result<String, ConfigError> {
        Ok(serde_yaml::to_string(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CONFIG: &str = r"
cluster:
  name: test-cluster
  bootnode:
    name: bootnode
    api_url: http://localhost:1633
  nodes:
    - name: bee-0
      api_url: http://localhost:1634
    - name: bee-1
      api_url: http://localhost:1635

checks:
  pingpong:
    enabled: true
    timeout: 5m
    retries: 3
  peercount:
    enabled: false
";

    const SAMPLE_CONFIG_WITH_TYPES: &str = r"
cluster:
  name: test-cluster
  bootnode:
    name: bootnode
    api_url: http://localhost:1633
    node_type: boot
  nodes:
    - name: bee-full-0
      api_url: http://localhost:1634
      node_type: full
    - name: bee-light-0
      api_url: http://localhost:1635
      node_type: light
";

    #[test]
    fn test_parse_config() {
        let config = Config::from_yaml(SAMPLE_CONFIG).unwrap();
        assert_eq!(config.cluster.name, "test-cluster");
        assert_eq!(config.cluster.bootnode.name, "bootnode");
        assert_eq!(config.cluster.nodes.len(), 2);
    }

    #[test]
    fn test_parse_config_with_node_types() {
        let config = Config::from_yaml(SAMPLE_CONFIG_WITH_TYPES).unwrap();
        assert_eq!(config.cluster.bootnode.node_type, NodeType::Boot);
        assert_eq!(config.cluster.nodes[0].node_type, NodeType::Full);
        assert_eq!(config.cluster.nodes[1].node_type, NodeType::Light);
    }

    #[test]
    fn test_node_type_defaults_to_full() {
        let config = Config::from_yaml(SAMPLE_CONFIG).unwrap();
        // Nodes without explicit type should default to Full
        assert_eq!(config.cluster.bootnode.node_type, NodeType::Full);
        assert_eq!(config.cluster.nodes[0].node_type, NodeType::Full);
        assert_eq!(config.cluster.nodes[1].node_type, NodeType::Full);
    }

    #[test]
    fn test_node_type_methods() {
        assert!(NodeType::Boot.is_boot());
        assert!(NodeType::Boot.is_full()); // Boot nodes are full-capable
        assert!(!NodeType::Boot.is_light());

        assert!(!NodeType::Full.is_boot());
        assert!(NodeType::Full.is_full());
        assert!(!NodeType::Full.is_light());

        assert!(!NodeType::Light.is_boot());
        assert!(!NodeType::Light.is_full());
        assert!(NodeType::Light.is_light());
    }

    #[test]
    fn test_node_type_display() {
        assert_eq!(NodeType::Boot.to_string(), "boot");
        assert_eq!(NodeType::Full.to_string(), "full");
        assert_eq!(NodeType::Light.to_string(), "light");
    }

    #[test]
    fn test_check_enabled() {
        let config = Config::from_yaml(SAMPLE_CONFIG).unwrap();
        assert!(config.is_check_enabled("pingpong"));
        assert!(!config.is_check_enabled("peercount"));
        assert!(config.is_check_enabled("unknown")); // Default to enabled
    }

    #[test]
    fn test_default_config() {
        let config = Config::default_config();
        assert!(!config.cluster.nodes.is_empty());
        assert!(!config.checks.is_empty());
        // Default config should have proper node types
        assert_eq!(config.cluster.bootnode.node_type, NodeType::Boot);
        assert_eq!(config.cluster.nodes[0].node_type, NodeType::Full);
    }

    #[test]
    fn test_roundtrip() {
        let config = Config::default_config();
        let yaml = config.to_yaml().unwrap();
        let parsed = Config::from_yaml(&yaml).unwrap();
        assert_eq!(config.cluster.name, parsed.cluster.name);
        assert_eq!(
            config.cluster.bootnode.node_type,
            parsed.cluster.bootnode.node_type
        );
    }
}
