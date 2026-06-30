//! Recording service trait and implementation

#[cfg(test)]
mod tests;

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ServiceResult;

/// Recording metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub user_id: Uuid,
    pub device_name: String,
    pub started_at: DateTime<Utc>,
    pub duration_secs: u64,
    pub file_size_bytes: u64,
    pub file_path: String,
    pub status: RecordingStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecordingStatus {
    Recording,
    Paused,
    Completed,
    Failed(String),
}

/// Audio format configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub bit_depth: u16,
    pub format: AudioFormat,
    pub chunk_duration_secs: u64,
    pub max_file_size_mb: Option<u64>,
    pub storage_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum AudioFormat {
    #[default]
    Wav,
    Flac,
    Mp3 { bitrate_kbps: u32 },
    Ogg { quality: i32 },
}

/// Recording session state
#[derive(Debug, Clone)]
pub struct RecordingSession {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub config: RecordingConfig,
    pub started_at: DateTime<Utc>,
    pub paused: bool,
    pub chunks_written: u64,
    pub current_file_path: String,
}

/// Recording service trait - defines the interface
#[async_trait]
pub trait RecordingService: Send + Sync {
    /// Start a new recording session
    async fn start_recording(
        &self,
        meeting_id: Uuid,
        user_id: Uuid,
        config: Option<RecordingConfig>,
    ) -> ServiceResult<RecordingSession>;

    /// Pause an active recording
    async fn pause_recording(&self, session_id: Uuid) -> ServiceResult<()>;

    /// Resume a paused recording
    async fn resume_recording(&self, session_id: Uuid) -> ServiceResult<()>;

    /// Stop recording and finalize file
    async fn stop_recording(&self, session_id: Uuid) -> ServiceResult<RecordingMetadata>;

    /// Write audio chunk to current file
    async fn write_chunk(
        &self,
        session_id: Uuid,
        audio_data: Bytes,
        timestamp: DateTime<Utc>,
    ) -> ServiceResult<u64>;

    /// Get recording session status
    async fn get_session_status(&self, session_id: Uuid) -> ServiceResult<RecordingSession>;

    /// Recover session after crash
    async fn recover_session(&self, session_id: Uuid) -> ServiceResult<RecordingSession>;

    /// Get recording metadata by ID
    async fn get_recording(&self, recording_id: Uuid) -> ServiceResult<RecordingMetadata>;

    /// List recordings for a meeting
    async fn list_recordings(&self, meeting_id: Uuid) -> ServiceResult<Vec<RecordingMetadata>>;

    /// Delete recording (file + metadata)
    async fn delete_recording(&self, recording_id: Uuid) -> ServiceResult<()>;
}

use crate::config::StorageConfig;

pub struct RecordingServiceImpl;

impl RecordingServiceImpl {
    pub fn new(_config: StorageConfig, _db_pool: sqlx::PgPool) -> Self {
        Self
    }
}

#[async_trait]
impl RecordingService for RecordingServiceImpl {
    async fn start_recording(
        &self,
        _meeting_id: Uuid,
        _user_id: Uuid,
        _config: Option<RecordingConfig>,
    ) -> ServiceResult<RecordingSession> {
        todo!("recording service is not implemented yet")
    }

    async fn pause_recording(&self, _session_id: Uuid) -> ServiceResult<()> {
        todo!("recording service is not implemented yet")
    }

    async fn resume_recording(&self, _session_id: Uuid) -> ServiceResult<()> {
        todo!("recording service is not implemented yet")
    }

    async fn stop_recording(&self, _session_id: Uuid) -> ServiceResult<RecordingMetadata> {
        todo!("recording service is not implemented yet")
    }

    async fn write_chunk(
        &self,
        _session_id: Uuid,
        _audio_data: Bytes,
        _timestamp: DateTime<Utc>,
    ) -> ServiceResult<u64> {
        todo!("recording service is not implemented yet")
    }

    async fn get_session_status(&self, _session_id: Uuid) -> ServiceResult<RecordingSession> {
        todo!("recording service is not implemented yet")
    }

    async fn recover_session(&self, _session_id: Uuid) -> ServiceResult<RecordingSession> {
        todo!("recording service is not implemented yet")
    }

    async fn get_recording(&self, _recording_id: Uuid) -> ServiceResult<RecordingMetadata> {
        todo!("recording service is not implemented yet")
    }

    async fn list_recordings(&self, _meeting_id: Uuid) -> ServiceResult<Vec<RecordingMetadata>> {
        todo!("recording service is not implemented yet")
    }

    async fn delete_recording(&self, _recording_id: Uuid) -> ServiceResult<()> {
        todo!("recording service is not implemented yet")
    }
}
