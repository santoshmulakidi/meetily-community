//! OpenAPI schema definition for Meetily Community+ API
//!
//! This module defines the complete API schema using utoipa annotations.
//! The generated OpenAPI spec is available at:
//! - JSON: `/api/v1/openapi.json`
//! - Swagger UI: `/swagger-ui`
//!
//! # Usage
//!
//! The schema is automatically generated from Rust types and endpoint annotations.
//! All request/response types derive `ToSchema`, and endpoints use `#[utoipa::path]`.

use utoipa::OpenApi;

/// Meetily Community+ API v1
///
/// A self-hosted AI meeting assistant providing:
/// - **Recording**: Unlimited audio recording with crash recovery
/// - **Transcription**: Multi-provider speech-to-text (Whisper, NVIDIA, faster-whisper)
/// - **Diarization**: Speaker identification and labeling
/// - **Summaries**: AI-generated meeting summaries (6 types)
/// - **Embeddings**: Vector embeddings for semantic search
/// - **Chat**: ChatGPT-style Q&A over meeting transcripts
/// - **Analytics**: Meeting statistics and insights
///
/// ## Authentication
///
/// Most endpoints require authentication via Bearer token in the `Authorization` header:
/// ```
/// Authorization: Bearer <your-api-key>
/// ```
///
/// Public endpoints (no auth required):
/// - `GET /health` - Health check
/// - `GET /api/v1/openapi.json` - OpenAPI schema
/// - `GET /swagger-ui` - Interactive API documentation
///
/// ## Rate Limiting
///
/// API calls are rate-limited to 100 requests per minute per API key.
///
/// ## Response Format
///
/// All responses use standard HTTP status codes:
/// - `200 OK` - Success
/// - `201 Created` - Resource created
/// - `400 Bad Request` - Invalid input
/// - `401 Unauthorized` - Missing or invalid authentication
/// - `404 Not Found` - Resource not found
/// - `500 Internal Server Error` - Server error
///
/// Error responses include a JSON body with details:
/// ```json
/// {
///   "error": "ErrorType",
///   "message": "Human-readable error message",
///   "details": {}
/// }
/// ```
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Meetily Community+ API",
        version = "1.0.0",
        description = "
## Meetily Community+ API

A self-hosted AI meeting assistant that transforms your audio meetings into actionable insights.

### Features

- **🎙️ Recording**: Unlimited audio recording with file rotation and crash recovery
- **📝 Transcription**: Multi-provider speech-to-text (Whisper, NVIDIA, faster-whisper)
- **👥 Diarization**: Automatic speaker identification and labeling
- **📊 Summaries**: AI-generated summaries (executive, technical, action items, decisions, risks, follow-up)
- **🧠 Embeddings**: Vector embeddings for semantic search across meetings
- **💬 Chat**: ChatGPT-style Q&A over your meeting transcripts with citations
- **📈 Analytics**: Meeting statistics, speaker analytics, and topic trends

### Quick Start

1. **Create a meeting**: `POST /api/v1/meetings`
2. **Start recording**: `POST /api/v1/recordings/start`
3. **Get transcript**: `GET /api/v1/transcripts/{meeting_id}`
4. **Generate summary**: `POST /api/v1/summaries/{meeting_id}`
5. **Chat about meeting**: `POST /api/v1/chat`

### Base URL

```
http://localhost:8080/api/v1
```
",
        license(
            name = "MIT",
        ),
        contact(
            name = "Meetily Community",
            url = "https://github.com/Zackriya-Solutions/meetily",
        ),
    ),
    tags(
        (name = "meetings", description = "Meeting management endpoints"),
        (name = "recordings", description = "Audio recording operations"),
        (name = "transcripts", description = "Speech-to-text transcription"),
        (name = "diarizations", description = "Speaker identification"),
        (name = "summaries", description = "AI-generated meeting summaries"),
        (name = "embeddings", description = "Vector embeddings for semantic search"),
        (name = "chat", description = "ChatGPT-style Q&A over meetings"),
        (name = "analytics", description = "Meeting statistics and insights"),
        (name = "health", description = "Health check and diagnostics"),
    ),
    paths(
        // Health endpoints
        crate::api::handlers::health::health_check,
        crate::api::handlers::health::health_detailed,
        
        // Meeting endpoints
        crate::api::handlers::meetings::create_meeting,
        crate::api::handlers::meetings::get_meeting,
        crate::api::handlers::meetings::list_meetings,
        crate::api::handlers::meetings::update_meeting,
        crate::api::handlers::meetings::delete_meeting,
        
        // Recording endpoints
        crate::api::handlers::recordings::start_recording,
        crate::api::handlers::recordings::stop_recording,
        crate::api::handlers::recordings::pause_recording,
        crate::api::handlers::recordings::resume_recording,
        crate::api::handlers::recordings::get_recording,
        crate::api::handlers::recordings::list_recordings,
        
        // Transcript endpoints
        crate::api::handlers::transcripts::get_transcript,
        crate::api::handlers::transcripts::generate_transcript,
        crate::api::handlers::transcripts::list_segments,
        
        // Diarization endpoints
        crate::api::handlers::diarizations::generate_diarization,
        crate::api::handlers::diarizations::get_diarization,
        crate::api::handlers::diarizations::update_speaker_alias,
        
        // Summary endpoints
        crate::api::handlers::summaries::generate_summary,
        crate::api::handlers::summaries::get_summary,
        crate::api::handlers::summaries::list_summaries,
        
        // Embedding endpoints
        crate::api::handlers::embeddings::create_embeddings,
        crate::api::handlers::embeddings::search_embeddings,
        
        // Chat endpoints
        crate::api::handlers::chat::send_message,
        crate::api::handlers::chat::get_conversation,
        crate::api::handlers::chat::list_conversations,
        
        // Analytics endpoints
        crate::api::handlers::analytics::get_dashboard,
        crate::api::handlers::analytics::get_meeting_stats,
    ),
    components(
        schemas(
            // Meeting types
            crate::api::handlers::meetings::CreateMeetingRequest,
            crate::api::handlers::meetings::MeetingResponse,
            crate::api::handlers::meetings::ListMeetingsResponse,
            
            // Recording types
            crate::api::handlers::recordings::StartRecordingRequest,
            crate::api::handlers::recordings::RecordingResponse,
            
            // Transcript types
            crate::api::handlers::transcripts::TranscriptResponse,
            crate::api::handlers::transcripts::TranscriptSegmentResponse,
            
            // Summary types
            crate::api::handlers::summaries::GenerateSummaryRequest,
            crate::api::handlers::summaries::SummaryResponse,
            
            // Chat types
            crate::api::handlers::chat::ChatRequest,
            crate::api::handlers::chat::ChatResponse,
            
            // Analytics types
            crate::api::handlers::analytics::DashboardResponse,
            crate::api::handlers::analytics::MeetingStats,
            
            // Common types
            crate::error::ErrorResponse,
        ),
    ),
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

/// Security scheme addon for OpenAPI
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "ApiKeyAuth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Bearer,
                    ),
                ),
            );
        }
    }
}

/// Health check response
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    /// Service status
    pub status: String,
    /// Service version
    pub version: String,
    /// Current timestamp
    pub timestamp: String,
}

/// Detailed health check with dependency status
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct HealthDetailedResponse {
    /// Overall status
    pub status: String,
    /// Database connection status
    pub database: String,
    /// Storage status
    pub storage: String,
    /// Active providers
    pub providers: Vec<String>,
}