//! Shared application state for Axum handlers

use axum::extract::FromRef;
use std::sync::Arc;

use crate::di::AppState;

/// Shared state accessible from all handlers
#[derive(Clone)]
pub struct SharedState {
    pub app_state: Arc<AppState>,
}

impl FromRef<AppState> for SharedState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            app_state: Arc::new(state.clone()),
        }
    }
}

impl SharedState {
    pub fn new(app_state: AppState) -> Self {
        Self {
            app_state: Arc::new(app_state),
        }
    }
}