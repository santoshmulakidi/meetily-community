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

use super::handlers::{health_handler, api_doc_handler};

/// Create the main application router
pub fn create_router(state: AppState) -> Router {
    // Build API routes
    let api_routes = Router::new()
        .route("/health", get(health_handler))
        .route("/api-docs/openapi.json", get(api_doc_handler))
        // Meeting routes
        .route("/api/v1/meetings", post(handlers::meetings::create_meeting))
        .route("/api/v1/meetings", get(handlers::meetings::list_meetings))
        .route("/api/v1/meetings/{id}", get(handlers::meetings::get_meeting))
        .route("/api/v1/meetings/{id}", put(handlers::meetings::update_meeting))
        .route("/api/v1/meetings/{id}", delete(handlers::meetings::delete_meeting))
        // Recording routes
        .route("/api/v1/recordings", post(handlers::recordings::create_recording))
        .route("/api/v1/recordings/{id}", get(handlers::recordings::get_recording))
        .route("/api/v1/recordings/{id}/pause", post(handlers::recordings::pause_recording))
        .route("/api/v1/recordings/{id}/resume", post(handlers::recordings::resume_recording))
        .route("/api/v1/recordings/{id}/stop", post(handlers::recordings::stop_recording))
        // Transcript routes
        .route("/api/v1/transcripts", get(handlers::transcripts::list_transcripts))
        .route("/api/v1/transcripts/{id}", get(handlers::transcripts::get_transcript))
        .route("/api/v1/transcripts/{id}", put(handlers::transcripts::update_transcript))
        // Summary routes
        .route("/api/v1/summaries", post(handlers::summaries::generate_summary))
        .route("/api/v1/summaries/{id}", get(handlers::summaries::get_summary))
        .route("/api/v1/summaries/{id}", put(handlers::summaries::update_summary))
        .route("/api/v1/summaries/{id}/export", get(handlers::summaries::export_summary))
        // Search routes
        .route("/api/v1/search", post(handlers::search::semantic_search))
        // Chat routes
        .route("/api/v1/chat", post(handlers::chat::chat))
        // Analytics routes
        .route("/api/v1/analytics", get(handlers::analytics::get_analytics));

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
#[openapi(
    paths(
        handlers::meetings::create_meeting,
        handlers::meetings::list_meetings,
        handlers::meetings::get_meeting,
        handlers::summaries::generate_summary,
        handlers::search::semantic_search,
        handlers::chat::chat,
    ),
    components(
        schemas(
            handlers::meetings::MeetingDto,
            handlers::summaries::MeetingSummaryDto,
        )
    ),
    tags(
        (name = "Meetings", description = "Meeting management endpoints"),
        (name = "Recordings", description = "Audio recording endpoints"),
        (name = "Transcripts", description = "Transcription endpoints"),
        (name = "Summaries", description = "AI summary generation endpoints"),
        (name = "Search", description = "Semantic search endpoints"),
        (name = "Chat", description = "RAG chat over meetings"),
        (name = "Analytics", description = "Meeting analytics"),
    )
)]
pub struct ApiDoc;