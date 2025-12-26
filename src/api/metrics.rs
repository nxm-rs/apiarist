//! Prometheus metrics for Kurtosis observability
//!
//! Exposes metrics in Prometheus text format at `/metrics`.
//! This integrates with the observability stack deployed by apiary.
//!
//! ## Metrics Exposed
//!
//! - `apiarist_checks_total` - Total number of checks configured
//! - `apiarist_checks_completed` - Number of checks completed
//! - `apiarist_checks_passed` - Number of checks passed
//! - `apiarist_checks_failed` - Number of checks failed
//! - `apiarist_check_duration_seconds` - Duration of each check
//! - `apiarist_execution_status` - Current execution status (0=running, 1=completed, 2=failed)
//!
//! ## Kurtosis Integration
//!
//! The observability stack in apiary can scrape these metrics:
//!
//! ```yaml
//! scrape_configs:
//!   - job_name: 'apiarist'
//!     static_configs:
//!       - targets: ['apiarist:8080']
//!     metrics_path: '/metrics'
//! ```

use axum::{extract::State, http::StatusCode, response::IntoResponse};
use std::fmt::Write;

use super::state::{ApiState, ExecutionStatus};

/// Generate Prometheus-format metrics
pub async fn metrics_handler(State(state): State<ApiState>) -> impl IntoResponse {
    let status_response = state.get_status_response();
    let results = state.get_results();

    let mut output = String::new();

    // Metadata
    writeln!(output, "# HELP apiarist_info Apiarist build information").unwrap();
    writeln!(output, "# TYPE apiarist_info gauge").unwrap();
    writeln!(output, "apiarist_info{{version=\"0.1.0\"}} 1").unwrap();
    writeln!(output).unwrap();

    // Execution status (gauge: 0=running, 1=completed, 2=failed)
    writeln!(
        output,
        "# HELP apiarist_execution_status Current execution status (0=running, 1=completed, 2=failed)"
    )
    .unwrap();
    writeln!(output, "# TYPE apiarist_execution_status gauge").unwrap();
    let status_value = match status_response.status {
        ExecutionStatus::Running => 0,
        ExecutionStatus::Completed => 1,
        ExecutionStatus::Failed => 2,
    };
    writeln!(output, "apiarist_execution_status {}", status_value).unwrap();
    writeln!(output).unwrap();

    // Checks total
    writeln!(
        output,
        "# HELP apiarist_checks_total Total number of checks configured"
    )
    .unwrap();
    writeln!(output, "# TYPE apiarist_checks_total gauge").unwrap();
    writeln!(output, "apiarist_checks_total {}", status_response.checks_total).unwrap();
    writeln!(output).unwrap();

    // Checks completed
    writeln!(
        output,
        "# HELP apiarist_checks_completed Number of checks completed"
    )
    .unwrap();
    writeln!(output, "# TYPE apiarist_checks_completed gauge").unwrap();
    writeln!(
        output,
        "apiarist_checks_completed {}",
        status_response.checks_completed
    )
    .unwrap();
    writeln!(output).unwrap();

    // Checks passed
    writeln!(
        output,
        "# HELP apiarist_checks_passed Number of checks that passed"
    )
    .unwrap();
    writeln!(output, "# TYPE apiarist_checks_passed gauge").unwrap();
    writeln!(
        output,
        "apiarist_checks_passed {}",
        status_response.checks_passed
    )
    .unwrap();
    writeln!(output).unwrap();

    // Checks failed
    writeln!(
        output,
        "# HELP apiarist_checks_failed Number of checks that failed"
    )
    .unwrap();
    writeln!(output, "# TYPE apiarist_checks_failed gauge").unwrap();
    writeln!(
        output,
        "apiarist_checks_failed {}",
        status_response.checks_failed
    )
    .unwrap();
    writeln!(output).unwrap();

    // Elapsed time
    writeln!(
        output,
        "# HELP apiarist_elapsed_seconds Time elapsed since start"
    )
    .unwrap();
    writeln!(output, "# TYPE apiarist_elapsed_seconds gauge").unwrap();
    writeln!(
        output,
        "apiarist_elapsed_seconds {}",
        status_response.elapsed_ms as f64 / 1000.0
    )
    .unwrap();
    writeln!(output).unwrap();

    // Per-check results
    if !results.is_empty() {
        writeln!(
            output,
            "# HELP apiarist_check_passed Whether a specific check passed (1) or failed (0)"
        )
        .unwrap();
        writeln!(output, "# TYPE apiarist_check_passed gauge").unwrap();
        for result in &results {
            let passed = if result.passed { 1 } else { 0 };
            writeln!(
                output,
                "apiarist_check_passed{{check=\"{}\"}} {}",
                result.check_name, passed
            )
            .unwrap();
        }
        writeln!(output).unwrap();

        writeln!(
            output,
            "# HELP apiarist_check_duration_seconds Duration of each check"
        )
        .unwrap();
        writeln!(output, "# TYPE apiarist_check_duration_seconds gauge").unwrap();
        for result in &results {
            writeln!(
                output,
                "apiarist_check_duration_seconds{{check=\"{}\"}} {}",
                result.check_name,
                result.duration.as_secs_f64()
            )
            .unwrap();
        }
        writeln!(output).unwrap();

        // Node results per check
        writeln!(
            output,
            "# HELP apiarist_check_nodes_passed Number of nodes that passed for each check"
        )
        .unwrap();
        writeln!(output, "# TYPE apiarist_check_nodes_passed gauge").unwrap();
        for result in &results {
            let passed_count = result.node_results.iter().filter(|r| r.passed).count();
            writeln!(
                output,
                "apiarist_check_nodes_passed{{check=\"{}\"}} {}",
                result.check_name, passed_count
            )
            .unwrap();
        }
        writeln!(output).unwrap();

        writeln!(
            output,
            "# HELP apiarist_check_nodes_total Total number of nodes tested for each check"
        )
        .unwrap();
        writeln!(output, "# TYPE apiarist_check_nodes_total gauge").unwrap();
        for result in &results {
            writeln!(
                output,
                "apiarist_check_nodes_total{{check=\"{}\"}} {}",
                result.check_name,
                result.node_results.len()
            )
            .unwrap();
        }
    }

    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        output,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::CheckResult;
    use std::time::Duration;

    #[tokio::test]
    async fn test_metrics_format() {
        let state = ApiState::new();
        state.set_total_checks(2);

        // Simulate a completed check
        let result = CheckResult::new("pingpong", vec![], Duration::from_secs(5));
        state.record_result(result);
        state.complete(true);

        let response = metrics_handler(State(state)).await;
        let response = response.into_response();

        // Just verify it returns OK
        assert_eq!(response.status(), StatusCode::OK);
    }
}
