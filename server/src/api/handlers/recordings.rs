//! Recordings handlers - placeholders

use axum::{extract::{Path, State}, http::StatusCode};
use uuid::Uuid;

use crate::api::state::SharedState;
use crate::error::AppError;

pub async fn create_recording(
    State(_state): State<SharedState>,
) -> Result<StatusCode, AppError> {
    todo!("Implement in Phase 3")
}

pub async fn get_recording(
    State(_state): State<SharedState>,
    Path(_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    todo!()
}

pub async fn pause_recording(
    State(_state): State<SharedState>,
    Path(_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    todo!()
}

pub async fn resume_recording(
    State(_state): State<SharedState>,
    Path(_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    todo!()
}

pub async fn stop_recording(
    State(_state): State<SharedState>,
    Path(_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    todo!()
}
