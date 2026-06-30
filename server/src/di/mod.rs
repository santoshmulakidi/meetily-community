//! Dependency Injection container
//!
//! Simple service locator pattern for managing application state.

use std::sync::Arc;

use crate::config::AppConfig;
use crate::services::recording::RecordingService;
use crate::services::transcription::TranscriptionService;
use crate::services::diarization::DiarizationService;
use crate::services::summary::SummaryService;
use crate::repositories::meeting::MeetingRepository;
use crate::repositories::embedding::EmbeddingRepository;

/// Application state shared across all handlers
/// Acts as a simple DI container
#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub recording_service: Arc<dyn RecordingService>,
    pub transcription_service: Arc<dyn TranscriptionService>,
    pub diarization_service: Arc<dyn DiarizationService>,
    pub summary_service: Arc<dyn SummaryService>,
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
        meeting_repo: Arc<dyn MeetingRepository>,
        embedding_repo: Arc<dyn EmbeddingRepository>,
    ) -> Self {
        Self {
            config,
            recording_service,
            transcription_service,
            diarization_service,
            summary_service,
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
