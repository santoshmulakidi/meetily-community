//! Diarization service trait and implementation

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::error::ServiceResult;
use crate::services::transcription::Transcript;

/// Speaker segment in transcript
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerSegment {
    pub speaker_id: String,
    pub speaker_label: String,  // e.g., "Speaker 1", "Alice"
    pub start_time_secs: f32,
    pub end_time_secs: f32,
    pub duration_secs: f32,
    pub confidence: f32,
}

/// Diarization result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiarizationResult {
    pub transcript_id: Uuid,
    pub segments: Vec<SpeakerSegment>,
    pub speaker_map: Vec<SpeakerInfo>,
    pub processing_time_secs: f32,
    pub model_used: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerInfo {
    pub speaker_id: String,
    pub total_duration_secs: f32,
    pub utterance_count: u32,
    pub avg_confidence: f32,
    pub embedding: Option<Vec<f32>>,  // For speaker verification
}

/// Diarization provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiarizationProvider {
    Pyannote { model: String },
    WhisperX,
    NVIDIANeMo { model_name: String },
    Custom { endpoint: String },
}

/// Diarization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiarizationConfig {
    pub provider: DiarizationProvider,
    pub num_speakers: Option<u32>,  // None = auto-detect
    pub min_speakers: u32,
    pub max_speakers: u32,
    pub use_embaddings: bool,
}

/// Diarization service trait
#[async_trait]
pub trait DiarizationService: Send + Sync {
    /// Apply diarization to existing transcript
    async fn diarize(
        &self,
        transcript_id: Uuid,
        config: Option<DiarizationConfig>,
    ) -> ServiceResult<DiarizationResult>;

    /// Apply diarization to audio file directly
    async fn diarize_audio(
        &self,
        audio_path: PathBuf,
        transcript: Transcript,
        config: Option<DiarizationConfig>,
    ) -> ServiceResult<DiarizationResult>;

    /// Assign speaker name/label
    async fn assign_speaker_name(
        &self,
        transcript_id: Uuid,
        speaker_id: String,
        name: String,
    ) -> ServiceResult<()>;

    /// Get speaker statistics
    async fn get_speaker_stats(&self, transcript_id: Uuid) -> ServiceResult<Vec<SpeakerInfo>>;

    /// Merge transcripts with speaker consistency
    async fn merge_with_speaker_consistency(
        &self,
        transcript_ids: Vec<Uuid>,
    ) -> ServiceResult<Transcript>;
}

pub struct DiarizationServiceImpl;

impl DiarizationServiceImpl {
    pub fn new(_db_pool: sqlx::PgPool) -> Self {
        Self
    }
}

#[async_trait]
impl DiarizationService for DiarizationServiceImpl {
    async fn diarize(
        &self,
        _transcript_id: Uuid,
        _config: Option<DiarizationConfig>,
    ) -> ServiceResult<DiarizationResult> {
        todo!("diarization service is not implemented yet")
    }

    async fn diarize_audio(
        &self,
        _audio_path: PathBuf,
        _transcript: Transcript,
        _config: Option<DiarizationConfig>,
    ) -> ServiceResult<DiarizationResult> {
        todo!("diarization service is not implemented yet")
    }

    async fn assign_speaker_name(
        &self,
        _transcript_id: Uuid,
        _speaker_id: String,
        _name: String,
    ) -> ServiceResult<()> {
        todo!("diarization service is not implemented yet")
    }

    async fn get_speaker_stats(&self, _transcript_id: Uuid) -> ServiceResult<Vec<SpeakerInfo>> {
        Ok(vec![])
    }

    async fn merge_with_speaker_consistency(
        &self,
        _transcript_ids: Vec<Uuid>,
    ) -> ServiceResult<Transcript> {
        todo!("diarization service is not implemented yet")
    }
}
