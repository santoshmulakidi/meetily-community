//! Semantic Search API handler
//!
//! Endpoints:
//! - POST /api/v1/search - Semantic search with hybrid retrieval

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    Extension,
};
use serde::{Deserialize, Serialize};
use sqlx::Pool;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::handlers::auth::User;
use crate::error::{AppError, ServiceResult};
use crate::services::semantic_search::{
    SemanticSearchService, SearchRequest, SearchResponse,
};

/// Search state
#[derive(Clone)]
pub struct SearchState {
    pub db_pool: Arc<Pool<sqlx::Postgres>>,
    pub search_service: Arc<SemanticSearchService>,
}

/// Search endpoint request (with optional user filters)
#[derive(Debug, Deserialize)]
pub struct SearchEndpointRequest {
    #[serde(flatten)]
    pub search: SearchRequest,
    /// Override user ID (admin only)
    pub user_id: Option<Uuid>,
}

/// Search endpoint
///
/// POST /api/v1/search
#[utoipa::path(
    post,
    path = "/api/v1/search",
    tag = "search",
    security(("ApiKeyAuth" = [])),
    request_body = SearchEndpointRequest,
    responses(
        (status = 200, description = "Search results", body = SearchResponse),
        (status = 400, description = "Invalid query"),
        (status = 401, description = "Unauthorized"),
    ),
)]
pub async fn search(
    State(state): State<SearchState>,
    Extension(user): Extension<User>,
    Json(request): Json<SearchEndpointRequest>,
) -> ServiceResult<Json<SearchResponse>> {
    // For now, use authenticated user's context
    // In future, could search across user's meetings only
    
    let response = state.search_service
        .search(request.search)
        .await?;
    
    Ok(Json(response))
}

/// Simple search (no auth required, public meetings only)
#[derive(Debug, Deserialize)]
pub struct SimpleSearchRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[inline]
fn default_limit() -> usize { 10 }

/// Simple search endpoint (public)
///
/// GET /api/v1/search/simple?query=budget+cuts&limit=5
#[utoipa::path(
    get,
    path = "/api/v1/search/simple",
    tag = "search",
    params(
        ("query" = String, Query, description = "Search query"),
        ("limit" = usize, Query, optional, description = "Max results"),
    ),
    responses(
        (status = 200, description = "Search results", body = SearchResponse),
        (status = 400, description = "Invalid query"),
    ),
)]
pub async fn simple_search(
    State(state): State<SearchState>,
    query_params: axum::extract::Query<SimpleSearchRequest>,
) -> ServiceResult<Json<SearchResponse>> {
    let request = SearchRequest {
        query: query_params.query.clone(),
        filters: Default::default(),
        limit: query_params.limit,
        include_excerpts: true,
    };
    
    let response = state.search_service
        .search(request)
        .await?;
    
    Ok(Json(response))
}