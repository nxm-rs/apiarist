//! HTTP Server for Status API
//!
//! Axum-based HTTP server providing status endpoints.

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;

use super::state::{ApiState, ExecutionStatus, HealthResponse, ResultsResponse, StatusResponse};

/// Start the API server on the given port
///
/// Returns a handle that can be used to gracefully shutdown the server.
pub async fn start_api_server(
    port: u16,
    state: ApiState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = create_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;

    info!(port = port, "Starting status API server");

    axum::serve(listener, app).await?;

    Ok(())
}

/// Create the API router
fn create_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/status", get(status_handler))
        .route("/results", get(results_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Health check endpoint
///
/// Returns 200 if the service is running.
/// Kurtosis can use this with assertions on the status field.
async fn health_handler(State(state): State<ApiState>) -> Json<HealthResponse> {
    let status = state.status();
    Json(HealthResponse {
        healthy: true,
        status,
    })
}

/// Status endpoint
///
/// Returns current execution status and progress.
async fn status_handler(State(state): State<ApiState>) -> Json<StatusResponse> {
    Json(state.get_status_response())
}

/// Results endpoint
///
/// Returns all check results.
/// Returns 202 Accepted if checks are still running.
async fn results_handler(State(state): State<ApiState>) -> (StatusCode, Json<ResultsResponse>) {
    let status = state.status();
    let results = state.get_results();

    let status_code = match status {
        ExecutionStatus::Running => StatusCode::ACCEPTED,
        ExecutionStatus::Completed => StatusCode::OK,
        ExecutionStatus::Failed => StatusCode::OK, // Still return 200 for failed checks
    };

    (status_code, Json(ResultsResponse { status, results }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        let state = ApiState::new();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_status_endpoint() {
        let state = ApiState::new();
        state.set_total_checks(3);
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_results_running() {
        let state = ApiState::new();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/results")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should return 202 Accepted while running
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn test_results_completed() {
        let state = ApiState::new();
        state.complete(true);
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/results")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
