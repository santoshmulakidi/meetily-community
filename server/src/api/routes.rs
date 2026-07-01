//! Router creation for Axum

use axum::{
    routing::{get, post, put, delete},
    Router,
};
use tower_http::trace::TraceLayer;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::di::AppState;

use super::handlers;
use super::handlers::{health_handler, api_doc_handler};

/// Create the main application router
pub fn create_router(state: AppState) -> Router {
    // Build API routes
    let api_routes = Router::new()
        .route("/health", get(health_handler))
        .route("/api-docs/openapi.json", get(api_doc_handler))
        // Meeting routes
        .route("/meetings", post(handlers::meetings::create_meeting))
        .route("/meetings", get(handlers::meetings::list_meetings))
        .route("/meetings/{id}", get(handlers::meetings::get_meeting))
        .route("/meetings/{id}", put(handlers::meetings::update_meeting))
        .route("/meetings/{id}", delete(handlers::meetings::delete_meeting))
        // Recording routes
        .route("/recordings", post(handlers::recordings::create_recording))
        .route("/recordings/{id}", get(handlers::recordings::get_recording))
        .route("/recordings/{id}/pause", post(handlers::recordings::pause_recording))
        .route("/recordings/{id}/resume", post(handlers::recordings::resume_recording))
        .route("/recordings/{id}/stop", post(handlers::recordings::stop_recording))
        // Transcript routes
        .route("/transcripts", get(handlers::transcripts::list_transcripts))
        .route("/transcripts/{id}", get(handlers::transcripts::get_transcript))
        .route("/transcripts/{id}", put(handlers::transcripts::update_transcript))
        // Summary routes
        .route("/summaries", post(handlers::summaries::generate_summary))
        .route("/summaries/{id}", get(handlers::summaries::get_summary))
        .route("/summaries/{id}", put(handlers::summaries::update_summary))
        .route("/summaries/{id}/export", get(handlers::summaries::export_summary))
        // Search routes
        .route("/search", post(handlers::search::semantic_search))
        // Chat routes
        .route("/chat", post(handlers::chat::chat))
        // Analytics routes
        .route("/analytics", get(handlers::analytics::get_analytics));

    // Add Swagger UI for API documentation
    let swagger = SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDoc::openapi());

    // Build the full router with state and middleware
    Router::new()
        .merge(swagger)
        .nest("/api", api_routes)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// OpenAPI documentation
#[derive(OpenApi)]
#[openapi()]
pub struct ApiDoc;
