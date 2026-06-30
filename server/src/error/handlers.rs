//! Error response handlers for Axum

use super::error::ErrorResponse;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Handle panics in the API layer gracefully
pub async fn handle_panic(_: StatusCode) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": "Internal server error",
            "code": "INTERNAL_ERROR",
            "details": "An unexpected error occurred"
        })),
    )
}

/// Global error handler for uniform responses
pub trait ErrorToResponse {
    fn to_response(self) -> Response;
}

impl ErrorToResponse for axum::extract::rejection::JsonRejection {
    fn to_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid JSON: {}", self.body_text()),
                code: "INVALID_JSON".to_string(),
                details: None,
            }),
        )
            .into_response()
    }
}