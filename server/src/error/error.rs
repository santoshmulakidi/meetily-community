//! Unified error types for Meetily Community+ Server

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// Unified error type for all services and repositories
#[derive(Debug, Error)]
pub enum AppError {
    /// Service layer errors
    #[error("Service error: {0}")]
    ServiceError(String),

    /// Repository/database errors
    #[error("Repository error: {0}")]
    RepositoryError(String),

    /// Input validation errors
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Authentication failures
    #[error("Authentication error: {0}")]
    AuthError(String),

    /// Authorization failures
    #[error("Authorization error: {0}")]
    AuthorizationError(String),

    /// Resource not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Resource already exists (conflict)
    #[error("Conflict: {0}")]
    Conflict(String),

    /// External service failures (LLM, transcription providers)
    #[error("External service error: {provider} - {message}")]
    ExternalServiceError {
        provider: String,
        message: String,
    },

    /// IO operation failures
    #[error("IO error")]
    IoError(#[from] std::io::Error),

    /// Database driver errors
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// JSON serialization errors
    #[error("JSON error: {0}")]
    JsonError(String),

    /// Generic internal error
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound("Resource not found in database".to_string()),
            sqlx::Error::Database(db_err) => {
                AppError::DatabaseError(db_err.to_string())
            }
            _ => AppError::RepositoryError(err.to_string()),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::JsonError(err.to_string())
    }
}

/// Standard error response format (OpenAPI-compatible)
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
    /// Error code (for programmatic handling)
    pub code: String,
    /// Additional error details (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_response) = match &self {
            // Client errors (4xx)
            AppError::ValidationError(msg) => (
                StatusCode::BAD_REQUEST,
                ErrorResponse {
                    error: msg.clone(),
                    code: "VALIDATION_ERROR".to_string(),
                    details: None,
                },
            ),
            AppError::AuthError(msg) => (
                StatusCode::UNAUTHORIZED,
                ErrorResponse {
                    error: msg.clone(),
                    code: "UNAUTHORIZED".to_string(),
                    details: None,
                },
            ),
            AppError::AuthorizationError(msg) => (
                StatusCode::FORBIDDEN,
                ErrorResponse {
                    error: msg.clone(),
                    code: "FORBIDDEN".to_string(),
                    details: None,
                },
            ),
            AppError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                ErrorResponse {
                    error: msg.clone(),
                    code: "NOT_FOUND".to_string(),
                    details: None,
                },
            ),
            AppError::Conflict(msg) => (
                StatusCode::CONFLICT,
                ErrorResponse {
                    error: msg.clone(),
                    code: "CONFLICT".to_string(),
                    details: None,
                },
            ),
            // Server errors (5xx)
            AppError::ExternalServiceError { provider, message } => (
                StatusCode::BAD_GATEWAY,
                ErrorResponse {
                    error: format!("{}: {}", provider, message),
                    code: "EXTERNAL_SERVICE_ERROR".to_string(),
                    details: None,
                },
            ),
            // Default to internal server error
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorResponse {
                    error: self.to_string(),
                    code: "INTERNAL_ERROR".to_string(),
                    details: {
                        // In debug mode, include error details
                        #[cfg(debug_assertions)]
                        {
                            Some(serde_json::json!({ "debug": format!("{:?}", self) }))
                        }
                        #[cfg(not(debug_assertions))]
                        {
                            None
                        }
                    },
                },
            ),
        };

        (status, Json(error_response)).into_response()
    }
}

/// Type aliases for cleaner code
pub type ServiceResult<T> = Result<T, AppError>;
pub type RepositoryResult<T> = Result<T, AppError>;
pub type ApiResult<T> = Result<T, AppError>;

/// Helper macros for creating errors
#[macro_export]
macro_rules! service_error {
    ($msg:expr) => {
        $crate::error::AppError::ServiceError($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::AppError::ServiceError(format!($fmt, $($arg)*))
    };
}

#[macro_export]
macro_rules! not_found {
    ($resource:expr) => {
        $crate::error::AppError::NotFound(format!("{} not found", $resource))
    };
}

#[macro_export]
macro_rules! validation_error {
    ($msg:expr) => {
        $crate::error::AppError::ValidationError($msg.to_string())
    };
}