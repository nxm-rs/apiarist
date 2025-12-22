//! API State management
//!
//! Shared state for the status API, tracking check execution progress.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::checks::CheckResult;

/// Execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    /// Checks are currently running
    Running,
    /// All checks completed successfully
    Completed,
    /// Some checks failed
    Failed,
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionStatus::Running => write!(f, "running"),
            ExecutionStatus::Completed => write!(f, "completed"),
            ExecutionStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Internal mutable state
#[derive(Debug)]
struct InnerState {
    status: ExecutionStatus,
    started_at: Instant,
    completed_at: Option<Instant>,
    checks_total: usize,
    checks_completed: usize,
    checks_passed: usize,
    checks_failed: usize,
    current_check: Option<String>,
    results: Vec<CheckResult>,
}

/// Shared API state
#[derive(Debug, Clone)]
pub struct ApiState {
    inner: Arc<RwLock<InnerState>>,
}

impl Default for ApiState {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiState {
    /// Create new API state
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(InnerState {
                status: ExecutionStatus::Running,
                started_at: Instant::now(),
                completed_at: None,
                checks_total: 0,
                checks_completed: 0,
                checks_passed: 0,
                checks_failed: 0,
                current_check: None,
                results: Vec::new(),
            })),
        }
    }

    /// Set total number of checks to run
    pub fn set_total_checks(&self, total: usize) {
        let mut state = self.inner.write();
        state.checks_total = total;
    }

    /// Mark a check as starting
    pub fn start_check(&self, name: &str) {
        let mut state = self.inner.write();
        state.current_check = Some(name.to_string());
    }

    /// Record a check result
    pub fn record_result(&self, result: CheckResult) {
        let mut state = self.inner.write();
        if result.passed {
            state.checks_passed += 1;
        } else {
            state.checks_failed += 1;
        }
        state.checks_completed += 1;
        state.current_check = None;
        state.results.push(result);
    }

    /// Mark execution as complete
    pub fn complete(&self, success: bool) {
        let mut state = self.inner.write();
        state.status = if success {
            ExecutionStatus::Completed
        } else {
            ExecutionStatus::Failed
        };
        state.completed_at = Some(Instant::now());
        state.current_check = None;
    }

    /// Get current status
    pub fn status(&self) -> ExecutionStatus {
        self.inner.read().status
    }

    /// Get status summary for API response
    pub fn get_status_response(&self) -> StatusResponse {
        let state = self.inner.read();
        StatusResponse {
            status: state.status,
            checks_total: state.checks_total,
            checks_completed: state.checks_completed,
            checks_passed: state.checks_passed,
            checks_failed: state.checks_failed,
            current_check: state.current_check.clone(),
            elapsed_ms: state.started_at.elapsed().as_millis() as u64,
            duration_ms: state.completed_at.map(|t| {
                t.duration_since(state.started_at).as_millis() as u64
            }),
        }
    }

    /// Get all results
    pub fn get_results(&self) -> Vec<CheckResult> {
        self.inner.read().results.clone()
    }

    /// Check if execution is complete
    pub fn is_complete(&self) -> bool {
        let state = self.inner.read();
        state.status != ExecutionStatus::Running
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.inner.read().started_at.elapsed()
    }
}

/// Status API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub status: ExecutionStatus,
    pub checks_total: usize,
    pub checks_completed: usize,
    pub checks_passed: usize,
    pub checks_failed: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_check: Option<String>,
    pub elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

/// Health response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub healthy: bool,
    pub status: ExecutionStatus,
}

/// Results response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultsResponse {
    pub status: ExecutionStatus,
    pub results: Vec<CheckResult>,
}
