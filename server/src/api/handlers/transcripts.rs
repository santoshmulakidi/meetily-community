//! Transcripts handlers - placeholders

use axum::{extract::{Path, State}, http::StatusCode};
use uuid::Uuid;

use crate::api::state::SharedState;
use crate::error::AppError;

pub async fn list_transcripts(
    State(_state): State<SharedState>,
) -> Result<StatusCode, AppError> {
    todo!("Implement in Phase 4")
}

pub async fn get_transcript(
    State(_state): State<SharedState>,
    Path(_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    todo!()
}

pub async fn update_transcript(
    State(_state): State<SharedState>,
    Path(_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    todo!()
}
