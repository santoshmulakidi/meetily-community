//! Meetily Community+ Server
//!
//! A self-hosted AI meeting assistant with:
//! - Multi-user support with authentication
//! - PostgreSQL + pgvector for storage and semantic search
//! - Pluggable transcription providers (Whisper, WhisperX, NVIDIA, Parakeet)
//! - Speaker diarization
//! - AI-powered summaries with multiple LLM providers
//! - RAG-based chat over meeting history
//!
//! # Architecture
//!
//! ```text
//! API Layer (Axum + utoipa for OpenAPI)
//!     ↓
//! Service Layer (Traits + Implementations)
//!     ↓
//! Repository Layer (Database abstraction)
//!     ↓
//! PostgreSQL + pgvector
//! ```

#![warn(rust_2018_idioms)]
#![warn(unused_crate_dependencies)]
#![deny(clippy::all)]

pub mod api;
pub mod config;
pub mod di;
pub mod error;
pub mod repositories;
pub mod services;

use anyhow::Result;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::AppConfig;
use crate::di::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing (structured logging)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "meetily_server=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false).with_thread_ids(false))
        .init();

    tracing::info!("🚀 Starting Meetily Community+ Server...");

    // Load configuration
    let config = AppConfig::from_env()?;
    tracing::info!("📋 Configuration loaded");

    // Initialize database connection pool
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .min_connections(config.database.min_connections)
        .connect(&config.database.url)
        .await?;
    tracing::info!("✅ Database connected");

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("✅ Database migrations completed");

    // Create service implementations
    // Note: We'll implement these in subsequent files
    let recording_service = Arc::new(services::recording::RecordingServiceImpl::new(
        config.storage.clone(),
        pool.clone(),
    ));

    let transcription_service = Arc::new(services::transcription::TranscriptionServiceImpl::new(
        config.transcription.clone(),
        pool.clone(),
    ));

    let diarization_service = Arc::new(services::diarization::DiarizationServiceImpl::new(
        pool.clone(),
    ));

    let summary_service = Arc::new(services::summary::SummaryServiceImpl::new(
        config.summary.clone(),
        pool.clone(),
    ));

    // Create repositories
    let meeting_repo = Arc::new(repositories::meeting::PostgresMeetingRepository::new(
        pool.clone(),
    ));

    let embedding_repo = Arc::new(repositories::embedding::PostgresEmbeddingRepository::new(
        pool.clone(),
    ));

    // Build application state (dependency injection container)
    let app_state = di::AppState::new(
        config.clone(),
        recording_service,
        transcription_service,
        diarization_service,
        summary_service,
        meeting_repo,
        embedding_repo,
    );

    // Build router
    let app = api::create_router(app_state);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    tracing::info!("🌐 Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}