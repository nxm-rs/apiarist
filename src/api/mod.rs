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

mod server;
mod state;

pub use server::start_api_server;
pub use state::{ApiState, ExecutionStatus};
