//! API handlers
//!
//! HTTP request handlers for all endpoints.

use axum::{http::StatusCode, Json};
use serde_json::json;

use crate::api::state::SharedState;

/// Health check endpoint
pub async fn health_handler() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "meetily-server"
    }))
}

/// API documentation endpoint
pub async fn api_doc_handler() -> &'static str {
    "OpenAPI JSON available at /swagger-ui"
}

// Re-export sub-modules
pub mod meetings;
pub mod recordings;
pub mod transcripts;
pub mod summaries;
pub mod search;
pub mod chat;
pub mod analytics;