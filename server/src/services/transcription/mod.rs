//! Transcription service trait and implementation

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::error::ServiceResult;

/// Transcription segment with timing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegment {
    pub id: Uuid,
    pub transcript_id: Uuid,
    pub start_time_secs: f32,
    pub end_time_secs: f32,
    pub text: String,
    pub confidence: f32,
    pub speaker_id: Option<String>,
    pub language: String,
    pub words: Vec<WordTiming>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordTiming {
    pub word: String,
    pub start_time_secs: f32,
    pub end_time_secs: f32,
    pub confidence: f32,
}

/// Complete transcript for a meeting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub recording_id: Option<Uuid>,
    pub segments: Vec<TranscriptionSegment>,
    pub language: String,
    pub duration_secs: f32,
    pub word_count: u32,
    pub status: TranscriptionStatus,
    pub metadata: TranscriptionMetadata,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TranscriptionStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranscriptionMetadata {
    pub model_name: String,
    pub provider: String,
    pub processing_time_secs: Option<f32>,
    pub gpu_accelerated: bool,
}

/// Transcription provider types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TranscriptionProvider {
    Whisper { model: WhisperModel },
    WhisperX { model: String, diarize: bool },
    Parakeet,
    NVIDIA { model_name: String },
    Custom { endpoint: String, model: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WhisperModel {
    Tiny,
    Base,
    Small,
    Medium,
    LargeV3,
    LargeV3Turbo,
}

/// Transcription configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionConfig {
    pub provider: TranscriptionProvider,
    pub language: Option<String>,  // None = auto-detect
    pub task: TranscriptionTask,
    pub conditions: Option<String>,
    pub hotwords: Option<Vec<String>>,
    pub chunk_duration_secs: u64,
    pub overlap_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum TranscriptionTask {
    #[default]
    Transcribe,
    Translate,  // Translate to English
}

/// Transcription service trait
#[async_trait]
pub trait TranscriptionService: Send + Sync {
    /// Transcribe audio file
    async fn transcribe_file(
        &self,
        recording_path: PathBuf,
        meeting_id: Uuid,
        config: Option<TranscriptionConfig>,
    ) -> ServiceResult<Transcript>;

    /// Transcribe audio stream (real-time)
    async fn transcribe_stream(
        &self,
        meeting_id: Uuid,
        config: Option<TranscriptionConfig>,
    ) -> ServiceResult<Uuid>;  // returns transcript_id

    /// Submit audio chunk for transcription
    async fn submit_chunk(
        &self,
        transcript_id: Uuid,
        audio_data: Bytes,
        offset_secs: f32,
    ) -> ServiceResult<TranscriptionSegment>;

    /// Get transcript by ID
    async fn get_transcript(&self, transcript_id: Uuid) -> ServiceResult<Transcript>;

    /// Get transcript for meeting
    async fn get_meeting_transcript(&self, meeting_id: Uuid) -> ServiceResult<Option<Transcript>>;

    /// Update transcript (e.g., after diarization)
    async fn update_transcript(
        &self,
        transcript_id: Uuid,
        segments: Vec<TranscriptionSegment>,
    ) -> ServiceResult<Transcript>;

    /// Delete transcript
    async fn delete_transcript(&self, transcript_id: Uuid) -> ServiceResult<()>;

    /// List available transcription models
    async fn list_models(&self) -> ServiceResult<Vec<String>>;
}

pub struct TranscriptionServiceImpl;

impl TranscriptionServiceImpl {
    pub fn new(_config: crate::config::TranscriptionConfig, _db_pool: sqlx::PgPool) -> Self {
        Self
    }
}

#[async_trait]
impl TranscriptionService for TranscriptionServiceImpl {
    async fn transcribe_file(
        &self,
        _recording_path: PathBuf,
        _meeting_id: Uuid,
        _config: Option<TranscriptionConfig>,
    ) -> ServiceResult<Transcript> {
        todo!("transcription service is not implemented yet")
    }

    async fn transcribe_stream(
        &self,
        _meeting_id: Uuid,
        _config: Option<TranscriptionConfig>,
    ) -> ServiceResult<Uuid> {
        todo!("transcription service is not implemented yet")
    }

    async fn submit_chunk(
        &self,
        _transcript_id: Uuid,
        _audio_data: Bytes,
        _offset_secs: f32,
    ) -> ServiceResult<TranscriptionSegment> {
        todo!("transcription service is not implemented yet")
    }

    async fn get_transcript(&self, _transcript_id: Uuid) -> ServiceResult<Transcript> {
        todo!("transcription service is not implemented yet")
    }

    async fn get_meeting_transcript(&self, _meeting_id: Uuid) -> ServiceResult<Option<Transcript>> {
        todo!("transcription service is not implemented yet")
    }

    async fn update_transcript(
        &self,
        _transcript_id: Uuid,
        _segments: Vec<TranscriptionSegment>,
    ) -> ServiceResult<Transcript> {
        todo!("transcription service is not implemented yet")
    }

    async fn delete_transcript(&self, _transcript_id: Uuid) -> ServiceResult<()> {
        todo!("transcription service is not implemented yet")
    }

    async fn list_models(&self) -> ServiceResult<Vec<String>> {
        Ok(vec![])
    }
}
