//! Dependency Injection container
//!
//! Simple service locator pattern for managing application state.

use std::sync::Arc;

use crate::config::AppConfig;
use crate::services::recording::RecordingService;
use crate::services::transcription::TranscriptionService;
use crate::services::diarization::DiarizationService;
use crate::services::summary::SummaryService;
use crate::services::embedding::EmbeddingService;
use crate::services::search::SearchService;
use crate::services::chat::ChatService;
use crate::repositories::meeting::MeetingRepository;
use crate::repositories::embedding::EmbeddingRepository;

/// Application state shared across all handlers
/// Acts as a simple DI container
pub struct AppState {
    pub config: AppConfig,
    pub recording_service: Arc<dyn RecordingService>,
    pub transcription_service: Arc<dyn TranscriptionService>,
    pub diarization_service: Arc<dyn DiarizationService>,
    pub summary_service: Arc<dyn SummaryService>,
    pub embedding_service: Arc<dyn EmbeddingService>,
    pub search_service: Arc<dyn SearchService>,
    pub chat_service: Arc<dyn ChatService>,
    pub meeting_repo: Arc<dyn MeetingRepository>,
    pub embedding_repo: Arc<dyn EmbeddingRepository>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: AppConfig,
        recording_service: Arc<dyn RecordingService>,
        transcription_service: Arc<dyn TranscriptionService>,
        diarization_service: Arc<dyn DiarizationService>,
        summary_service: Arc<dyn SummaryService>,
        embedding_service: Arc<dyn EmbeddingService>,
        search_service: Arc<dyn SearchService>,
        chat_service: Arc<dyn ChatService>,
        meeting_repo: Arc<dyn MeetingRepository>,
        embedding_repo: Arc<dyn EmbeddingRepository>,
    ) -> Self {
        Self {
            config,
            recording_service,
            transcription_service,
            diarization_service,
            summary_service,
            embedding_service,
            search_service,
            chat_service,
            meeting_repo,
            embedding_repo,
        }
    }
}

/// Builder for constructing AppState with dependencies
pub struct AppStateBuilder {
    config: Option<AppConfig>,
    recording_service: Option<Arc<dyn RecordingService>>,
    transcription_service: Option<Arc<dyn TranscriptionService>>,
    diarization_service: Option<Arc<dyn DiarizationService>>,
    summary_service: Option<Arc<dyn SummaryService>>,
    embedding_service: Option<Arc<dyn EmbeddingService>>,
    search_service: Option<Arc<dyn SearchService>>,
    chat_service: Option<Arc<dyn ChatService>>,
    meeting_repo: Option<Arc<dyn MeetingRepository>>,
    embedding_repo: Option<Arc<dyn EmbeddingRepository>>,
}

impl AppStateBuilder {
    pub fn new() -> Self {
        Self {
            config: None,
            recording_service: None,
            transcription_service: None,
            diarization_service: None,
            summary_service: None,
            embedding_service: None,
            search_service: None,
            chat_service: None,
            meeting_repo: None,
            embedding_repo: None,
        }
    }

    pub fn config(mut self, config: AppConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn recording_service(mut self, service: Arc<dyn RecordingService>) -> Self {
        self.recording_service = Some(service);
        self
    }

    pub fn transcription_service(mut self, service: Arc<dyn TranscriptionService>) -> Self {
        self.transcription_service = Some(service);
        self
    }

    pub fn diarization_service(mut self, service: Arc<dyn DiarizationService>) -> Self {
        self.diarization_service = Some(service);
        self
    }

    pub fn summary_service(mut self, service: Arc<dyn SummaryService>) -> Self {
        self.summary_service = Some(service);
        self
    }

    pub fn embedding_service(mut self, service: Arc<dyn EmbeddingService>) -> Self {
        self.embedding_service = Some(service);
        self
    }

    pub fn search_service(mut self, service: Arc<dyn SearchService>) -> Self {
        self.search_service = Some(service);
        self
    }

    pub fn chat_service(mut self, service: Arc<dyn ChatService>) -> Self {
        self.chat_service = Some(service);
        self
    }

    pub fn meeting_repo(mut self, repo: Arc<dyn MeetingRepository>) -> Self {
        self.meeting_repo = Some(repo);
        self
    }

    pub fn embedding_repo(mut self, repo: Arc<dyn EmbeddingRepository>) -> Self {
        self.embedding_repo = Some(repo);
        self
    }

    pub fn build(self) -> Result<AppState, String> {
        Ok(AppState::new(
            self.config.ok_or("config is required")?,
            self.recording_service.ok_or("recording_service is required")?,
            self.transcription_service.ok_or("transcription_service is required")?,
            self.diarization_service.ok_or("diarization_service is required")?,
            self.summary_service.ok_or("summary_service is required")?,
            self.embedding_service.ok_or("embedding_service is required")?,
            self.search_service.ok_or("search_service is required")?,
            self.chat_service.ok_or("chat_service is required")?,
            self.meeting_repo.ok_or("meeting_repo is required")?,
            self.embedding_repo.ok_or("embedding_repo is required")?,
        ))
    }
}

impl Default for AppStateBuilder {
    fn default() -> Self {
        Self::new()
    }
}