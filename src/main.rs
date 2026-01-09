//! Apiarist CLI - The Beekeeper Who Stress-Tests Your Swarm
//!
//! Run checks against Bee node clusters to verify network health and functionality.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use apiarist::api::{ApiState, start_api_server};
use apiarist::batch::prepare_batches;
use apiarist::checks::{Check, registry::CHECKS};
use apiarist::config::Config;

/// Apiarist - The beekeeper who stress-tests your Swarm
#[derive(Debug, Parser)]
#[command(name = "apiarist")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Output logs as JSON
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run checks against a Bee cluster
    Check {
        /// Path to cluster configuration file
        #[arg(short, long, default_value = "cluster.yaml")]
        config: String,

        /// Specific checks to run (comma-separated)
        #[arg(short = 'C', long)]
        checks: Option<String>,

        /// Start HTTP status API on this port
        #[arg(long)]
        api_port: Option<u16>,

        /// Timeout for all checks
        #[arg(short, long, default_value = "30m")]
        timeout: String,

        /// Keep running after checks complete (for API access)
        #[arg(long)]
        keep_alive: bool,
    },

    /// Generate a default configuration file
    Init {
        /// Output file path
        #[arg(short, long, default_value = "cluster.yaml")]
        output: String,
    },

    /// List available checks
    List,

    /// Validate a configuration file
    Validate {
        /// Path to configuration file
        #[arg(short, long, default_value = "cluster.yaml")]
        config: String,
    },
}

fn setup_logging(verbose: bool, json: bool) {
    let env_filter = if verbose {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"))
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    if json {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    setup_logging(cli.verbose, cli.json);

    match cli.command {
        Commands::Check {
            config: config_path,
            checks: check_filter,
            api_port,
            timeout: _timeout,
            keep_alive,
        } => run_checks(&config_path, check_filter.as_deref(), api_port, keep_alive).await,

        Commands::Init { output } => init_config(&output),

        Commands::List => {
            list_checks();
            Ok(())
        }

        Commands::Validate {
            config: config_path,
        } => validate_config(&config_path),
    }
}

/// Run checks against a cluster
async fn run_checks(
    config_path: &str,
    check_filter: Option<&str>,
    api_port: Option<u16>,
    keep_alive: bool,
) -> Result<()> {
    tracing::info!(config = %config_path, "Loading configuration");

    let config = Config::from_file(config_path)
        .with_context(|| format!("Failed to load config from {config_path}"))?;

    tracing::info!(
        cluster = %config.cluster.name,
        bootnode = %config.cluster.bootnode.name,
        nodes = config.cluster.nodes.len(),
        "Cluster configuration loaded"
    );

    // Create check context from cluster config
    let ctx = config
        .cluster
        .to_check_context()
        .context("Failed to create check context")?;

    // Check if any data checks will run (smoke, pushsync, retrieval)
    // If so, prepare batches upfront to avoid per-check batch creation delays
    let data_checks = ["smoke", "pushsync", "retrieval"];
    let needs_batches = check_filter
        .map(|f| f.split(',').any(|c| data_checks.contains(&c.trim())))
        .unwrap_or_else(|| data_checks.iter().any(|c| config.is_check_enabled(c)));

    if needs_batches {
        tracing::info!("Preparing postage batches on full nodes...");
        match prepare_batches(&ctx, None).await {
            Ok(result) => {
                tracing::info!(
                    created = result.batches_created,
                    reused = result.batches_reused,
                    duration_secs = result.duration.as_secs(),
                    "Batch preparation complete"
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "Batch preparation failed, checks may be slower");
            }
        }
    }

    // Determine which checks to run
    let checks_to_run: Vec<Arc<dyn Check>> = if let Some(filter) = check_filter {
        // Run specific checks
        filter
            .split(',')
            .filter_map(|name| {
                let name = name.trim();
                CHECKS.get(name).cloned().or_else(|| {
                    tracing::warn!(check = name, "Unknown check, skipping");
                    None
                })
            })
            .collect()
    } else {
        // Run all enabled checks from config
        CHECKS
            .iter()
            .filter(|(name, _)| config.is_check_enabled(name))
            .map(|(_, check)| check.clone())
            .collect()
    };

    if checks_to_run.is_empty() {
        tracing::warn!("No checks to run");
        return Ok(());
    }

    // Create API state
    let api_state = ApiState::new();
    api_state.set_total_checks(checks_to_run.len());

    // Start API server if port specified
    if let Some(port) = api_port {
        let state_clone = api_state.clone();
        tokio::spawn(async move {
            if let Err(e) = start_api_server(port, state_clone).await {
                tracing::error!(error = %e, "API server error");
            }
        });
        // Give the server a moment to start
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    tracing::info!(
        count = checks_to_run.len(),
        checks = ?checks_to_run.iter().map(|c| c.name()).collect::<Vec<_>>(),
        "Running checks in parallel"
    );

    // Run all checks in parallel
    let ctx = Arc::new(ctx);
    let config = Arc::new(config);

    let mut handles = Vec::new();
    for check in checks_to_run {
        let check_name = check.name().to_string();
        let ctx = Arc::clone(&ctx);
        let config = Arc::clone(&config);
        let api_state = api_state.clone();

        // Get options from config or use defaults
        let opts = config
            .check_config(&check_name)
            .map(|c| c.to_check_options(&check.default_options()))
            .unwrap_or_else(|| check.default_options());

        handles.push(tokio::spawn(async move {
            tracing::info!(check = %check_name, "Starting check");
            api_state.start_check(&check_name);

            let result = check.run(&ctx, &opts).await;
            (check_name, result, api_state)
        }));
    }

    // Wait for all checks to complete and collect results
    let mut all_passed = true;
    for handle in handles {
        match handle.await {
            Ok((check_name, result, api_state)) => {
                match result {
                    Ok(result) => {
                        let passed = result.passed;
                        if passed {
                            tracing::info!(
                                check = %check_name,
                                duration_ms = result.duration.as_millis(),
                                message = ?result.message,
                                "Check PASSED"
                            );
                        } else {
                            tracing::error!(
                                check = %check_name,
                                duration_ms = result.duration.as_millis(),
                                message = ?result.message,
                                failed_nodes = result.node_results.iter().filter(|r| !r.passed).count(),
                                "Check FAILED"
                            );
                            all_passed = false;
                        }

                        // Log individual node results at debug level
                        for node_result in &result.node_results {
                            if node_result.passed {
                                tracing::debug!(
                                    check = %check_name,
                                    node = %node_result.node,
                                    "Node passed"
                                );
                            } else {
                                tracing::warn!(
                                    check = %check_name,
                                    node = %node_result.node,
                                    error = ?node_result.error,
                                    "Node failed"
                                );
                            }
                        }

                        // Record result in API state
                        api_state.record_result(result);
                    }
                    Err(e) => {
                        tracing::error!(check = %check_name, error = %e, "Check error");
                        all_passed = false;
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Check task panicked");
                all_passed = false;
            }
        }
    }

    // Mark completion
    api_state.complete(all_passed);

    if all_passed {
        tracing::info!("All checks PASSED");
    } else {
        tracing::error!("Some checks FAILED");
    }

    // If keep_alive is set and API is running, wait forever
    if keep_alive && api_port.is_some() {
        tracing::info!("Keeping alive for API access. Press Ctrl+C to exit.");
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    }

    if all_passed {
        Ok(())
    } else {
        anyhow::bail!("Some checks FAILED")
    }
}

/// Generate a default configuration file
fn init_config(output: &str) -> Result<()> {
    let config = Config::default_config();
    let yaml = config.to_yaml().context("Failed to serialize config")?;

    std::fs::write(output, &yaml).with_context(|| format!("Failed to write config to {output}"))?;

    tracing::info!(path = %output, "Configuration file created");
    println!("Created {output}");
    println!();
    println!("Edit the file to configure your cluster, then run:");
    println!("  apiarist check --config {output}");

    Ok(())
}

/// List available checks
fn list_checks() {
    println!("Available checks:");
    println!();

    // Group checks by category (for now just list all)
    let mut checks: Vec<_> = CHECKS.iter().collect();
    checks.sort_by_key(|(name, _)| *name);

    for (name, check) in checks {
        println!("  {name:20} - {}", check.description());
    }

    println!();
    println!("Run specific checks with:");
    println!("  apiarist check --checks pingpong,peercount");
}

/// Validate a configuration file
fn validate_config(config_path: &str) -> Result<()> {
    tracing::info!(config = %config_path, "Validating configuration");

    let config = Config::from_file(config_path)
        .with_context(|| format!("Failed to load config from {config_path}"))?;

    println!("Configuration is valid!");
    println!();
    println!("Cluster: {}", config.cluster.name);
    println!(
        "Bootnode: {} ({})",
        config.cluster.bootnode.name, config.cluster.bootnode.api_url
    );
    println!("Nodes: {}", config.cluster.nodes.len());

    for node in &config.cluster.nodes {
        println!("  - {} ({})", node.name, node.api_url);
    }

    println!();
    println!("Checks configured: {}", config.checks.len());

    for (name, check_config) in &config.checks {
        let status = if check_config.enabled {
            "enabled"
        } else {
            "disabled"
        };
        println!("  - {name}: {status}");
    }

    Ok(())
}
