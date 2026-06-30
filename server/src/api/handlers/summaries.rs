//! Summaries handlers - placeholders

use axum::{extract::{Path, State}, http::StatusCode};
use uuid::Uuid;

use crate::api::state::SharedState;
use crate::error::AppError;

pub async fn generate_summary(
    State(_state): State<SharedState>,
) -> Result<StatusCode, AppError> {
    todo!("Implement in Phase 6")
}

pub async fn get_summary(
    State(_state): State<SharedState>,
    Path(_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    todo!()
}

pub async fn update_summary(
    State(_state): State<SharedState>,
    Path(_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    todo!()
}

pub async fn export_summary(
    State(_state): State<SharedState>,
    Path(_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    todo!()
}
