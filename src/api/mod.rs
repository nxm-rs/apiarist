//! Status HTTP API
//!
//! Provides an HTTP API for monitoring check execution status.
//! This allows Kurtosis and other tools to wait for check completion.
//!
//! ## Endpoints
//!
//! - `GET /health` - Health check (always returns 200 if running)
//! - `GET /status` - Current execution status
//! - `GET /results` - Check results (available after completion)
//! - `GET /metrics` - Prometheus metrics (for observability stack)
//!
//! ## Kurtosis Integration
//!
//! Primary interface is the status API (used with `plan.wait()`).
//! Metrics endpoint integrates with the observability stack:
//!
//! ```python
//! plan.wait(
//!     service_name = "apiarist",
//!     recipe = GetHttpRequestRecipe(port_id = "api", endpoint = "/status"),
//!     field = "extract.status",
//!     assertion = "IN",
//!     target_value = ["completed", "failed"],
//! )
//! ```

mod metrics;
mod server;
mod state;

pub use server::start_api_server;
pub use state::{ApiState, ExecutionStatus};
