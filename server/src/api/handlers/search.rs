//! Search handlers - placeholders

use axum::{extract::State, http::StatusCode};

use crate::api::state::SharedState;
use crate::error::AppError;

pub async fn semantic_search(
    State(_state): State<SharedState>,
) -> Result<StatusCode, AppError> {
    todo!("Implement search")
}
