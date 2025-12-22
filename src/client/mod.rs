//! Bee API Client
//!
//! Hand-written client for the Bee HTTP API.
//! Types are based on the OpenAPI specification in `openapi/Swarm.yaml` and `openapi/SwarmCommon.yaml`.

mod bee;
mod types;

pub use bee::{BeeClient, BeeError, BeeResult};
pub use types::*;
