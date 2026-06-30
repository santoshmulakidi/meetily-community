//! Recording service implementation
//!
//! Features:
//! - Unlimited recording with automatic file rotation
//! - Crash recovery with session checkpoints
//! - Pause/resume functionality
//! - Low-memory streaming writes
//! - Configurable storage location

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use uuid::Uuid;
use tracing::{info, warn, error, debug};

use crate::error::{AppError, ServiceResult};
use super::{RecordingService, RecordingSession, RecordingMetadata, RecordingStatus, RecordingConfig, AudioFormat};
use crate::config::StorageConfig;

/// Maximum chunk file size before rotation (100MB default)
const DEFAULT_MAX_CHUNK_SIZE_BYTES: u64 = 100 * 1024 * 1024;

/// Checkpoint interval (save state every N chunks)
const CHECKPOINT_INTERVAL: u64 = 10;

/// Recording service implementation
pub struct RecordingServiceImpl {
    config: StorageConfig,
    db_pool: Pool<Postgres>,
    /// Active recording sessions (in-memory state)
    sessions: Arc<RwLock<Vec<ActiveSession>>>,
}

/// Active recording session with runtime state
struct ActiveSession {
    session: RecordingSession,
    writer: Arc<Mutex<StreamingWriter>>,
    started_at: DateTime<Utc>,
    last_checkpoint: DateTime<Utc>,
    chunks_since_checkpoint: u64,
}

/// Streaming WAV writer with low-memory footprint
struct StreamingWriter {
    current_file: PathBuf,
    file: BufWriter<File>,
    bytes_written: u64,
    max_file_size: u64,
    chunk_index: u64,
    sample_rate: u32,
    channels: u16,
}

impl StreamingWriter {
    /// Create a new streaming writer
    async fn new(
        base_path: PathBuf,
        session_id: Uuid,
        config: &RecordingConfig,
        max_file_size: u64,
    ) -> ServiceResult<Self> {
        let chunk_file = base_path.join(format!("{}_chunk_0.wav", session_id));
        
        // Ensure directory exists
        if let Some(parent) = chunk_file.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                AppError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to create directory: {}", e)
                ))
            })?;
        }
        
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&chunk_file)
            .await
            .map_err(|e| {
                AppError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to create file: {}", e)
                ))
            })?;
        
        let mut writer = Self {
            current_file: chunk_file,
            file: BufWriter::new(file),
            bytes_written: 0,
            max_file_size,
            chunk_index: 0,
            sample_rate: config.sample_rate,
            channels: config.channels,
        };
        
        // Write WAV header (will be updated on close)
        writer.write_placeholder_header().await?;
        
        Ok(writer)
    }
    
    /// Write a placeholder WAV header (44 bytes)
    /// Real header written on close with actual file size
    async fn write_placeholder_header(&mut self) -> ServiceResult<()> {
        let header = vec![0u8; 44];
        self.file.write_all(&header).await.map_err(|e| {
            AppError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to write WAV header: {}", e)
            ))
        })?;
        self.bytes_written += 44;
        Ok(())
    }
    
    /// Write audio chunk (PCM data)
    async fn write_chunk(&mut self, audio_data: &[u8]) -> ServiceResult<u64> {
        // Check if we need to rotate files
        if self.bytes_written + audio_data.len() as u64 > self.max_file_size {
            self.rotate_file().await?;
        }
        
        self.file.write_all(audio_data).await.map_err(|e| {
            AppError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to write audio chunk: {}", e)
            ))
        })?;
        
        self.bytes_written += audio_data.len() as u64;
        Ok(self.bytes_written)
    }
    
    /// Rotate to a new chunk file
    async fn rotate_file(&mut self) -> ServiceResult<()> {
        // Flush current file
        self.file.flush().await.map_err(|e| {
            AppError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to flush file: {}", e)
            ))
        })?;
        
        // Update WAV header with actual size
        self.finalize_wav_header().await?;
        
        // Create new chunk file
        self.chunk_index += 1;
        let new_file = self.current_file.parent().unwrap().join(
            format!("{}_chunk_{}.wav", 
                self.current_file.file_stem().unwrap().to_string_lossy().replace("_chunk_0", "")
            )
        );
        
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&new_file)
            .await
            .map_err(|e| {
                AppError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to create new chunk file: {}", e)
                ))
            })?;
        
        self.current_file = new_file;
        self.file = BufWriter::new(file);
        self.bytes_written = 44; // Header
        
        // Write placeholder header for new file
        self.write_placeholder_header().await?;
        
        info!("Rotated recording file to {:?}", self.current_file);
        Ok(())
    }
    
    /// Finalize WAV header with actual file size
    async fn finalize_wav_header(&mut self) -> ServiceResult<()> {
        use std::io::SeekFrom;
        use tokio::io::AsyncSeekExt;
        
        let data_size = self.bytes_written - 44;
        let file_size = self.bytes_written;
        
        // Seek to beginning
        self.file.seek(SeekFrom::Start(0)).await.map_err(|e| {
            AppError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to seek: {}", e)
            ))
        })?;
        
        // Write proper WAV header
        let mut header = Vec::with_capacity(44);
        header.extend_from_slice(b"RIFF");
        header.extend_from_slice(&(file_size as u32 - 8).to_le_bytes());
        header.extend_from_slice(b"WAVE");
        header.extend_from_slice(b"fmt ");
        header.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        header.extend_from_slice(&1u16.to_le_bytes());  // PCM format
        header.extend_from_slice(&self.channels.to_le_bytes());
        header.extend_from_slice(&self.sample_rate.to_le_bytes());
        header.extend_from_slice(&(self.sample_rate * self.channels as u32).to_le_bytes());
        header.extend_from_slice(&(self.sample_rate * self.channels as u32).to_le_bytes());
        header.extend_from_slice(&(self.channels * 16 / 8).to_le_bytes()); // bits per sample
        header.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        header.extend_from_slice(b"data");
        header.extend_from_slice(&(data_size as u32).to_le_bytes());
        
        self.file.write_all(&header).await.map_err(|e| {
            AppError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to write WAV header: {}", e)
            ))
        })?;
        
        // Seek back to end
        self.file.seek(SeekFrom::End(0)).await.map_err(|e| {
            AppError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to seek to end: {}", e)
            ))
        })?;
        
        Ok(())
    }
    
    /// Close and finalize the file
    async fn close(mut self) -> ServiceResult<PathBuf> {
        self.finalize_wav_header().await?;
        self.file.flush().await.map_err(|e| {
            AppError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to flush file: {}", e)
            ))
        })?;
        
        let path = self.current_file.clone();
        Ok(path)
    }
}

impl RecordingServiceImpl {
    /// Create a new recording service
    pub fn new(config: StorageConfig, db_pool: Pool<Postgres>) -> Self {
        Self {
            config,
            db_pool,
            sessions: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Generate recording file path
    fn generate_recording_path(&self, session_id: Uuid) -> PathBuf {
        let date = Utc::now().format("%Y-%m-%d");
        PathBuf::from(&self.config.recordings_path)
            .join(date.to_string())
            .join(format!("{}.wav", session_id))
    }
    
    /// Save session checkpoint to database
    async fn save_checkpoint(&self, session: &RecordingSession) -> ServiceResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO recording_sessions (id, meeting_id, config_json, started_at, paused, chunks_written, current_file_path, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET
                paused = EXCLUDED.paused,
                chunks_written = EXCLUDED.chunks_written,
                current_file_path = EXCLUDED.current_file_path,
                updated_at = NOW()
            "#,
            session.id,
            session.meeting_id,
            serde_json::to_value(&session.config).map_err(|e| AppError::JsonError(e.to_string()))?,
            session.started_at,
            session.paused,
            session.chunks_written as i64,
            session.current_file_path,
            Utc::now()
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Load session from checkpoint
    async fn load_checkpoint(&self, session_id: Uuid) -> ServiceResult<Option<RecordingSession>> {
        let record = sqlx::query_as!(
            (Uuid, Uuid, serde_json::Value, DateTime<Utc>, bool, i64, String),
            r#"
            SELECT id, meeting_id, config_json, started_at, paused, chunks_written, current_file_path
            FROM recording_sessions
            WHERE id = $1
            "#,
            session_id
        )
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        match record {
            Some((id, meeting_id, config_json, started_at, paused, chunks_written, current_file_path)) => {
                let config: RecordingConfig = serde_json::from_value(config_json)
                    .map_err(|e| AppError::JsonError(e.to_string()))?;
                
                Ok(Some(RecordingSession {
                    id,
                    meeting_id,
                    config,
                    started_at,
                    paused,
                    chunks_written: chunks_written as u64,
                    current_file_path,
                }))
            }
            None => Ok(None),
        }
    }
}

#[async_trait]
impl RecordingService for RecordingServiceImpl {
    async fn start_recording(
        &self,
        meeting_id: Uuid,
        user_id: Uuid,
        config: Option<RecordingConfig>,
    ) -> ServiceResult<RecordingSession> {
        let config = config.unwrap_or_else(|| RecordingConfig {
            sample_rate: 48000,
            channels: 2,  // Stereo (mic + system)
            bit_depth: 16,
            format: AudioFormat::Wav,
            chunk_duration_secs: 5,
            max_file_size_mb: Some(100),
            storage_path: self.config.recordings_path.clone(),
        });
        
        let session_id = Uuid::new_v4();
        let recording_path = self.generate_recording_path(session_id);
        
        let session = RecordingSession {
            id: session_id,
            meeting_id,
            config: config.clone(),
            started_at: Utc::now(),
            paused: false,
            chunks_written: 0,
            current_file_path: recording_path.to_string_lossy().to_string(),
        };
        
        // Create streaming writer
        let max_file_size = config.max_file_size_mb
            .map(|mb| mb * 1024 * 1024)
            .unwrap_or(DEFAULT_MAX_CHUNK_SIZE_BYTES);
        
        let writer = StreamingWriter::new(
            PathBuf::from(&config.storage_path),
            session_id,
            &config,
            max_file_size,
        ).await?;
        
        let active_session = ActiveSession {
            session: session.clone(),
            writer: Arc::new(Mutex::new(writer)),
            started_at: Utc::now(),
            last_checkpoint: Utc::now(),
            chunks_since_checkpoint: 0,
        };
        
        // Add to active sessions
        {
            let mut sessions = self.sessions.write().await;
            sessions.push(active_session);
        }
        
        // Save initial checkpoint
        self.save_checkpoint(&session).await?;
        
        info!("Started recording session {} for meeting {}", session_id, meeting_id);
        Ok(session)
    }
    
    async fn pause_recording(&self, session_id: Uuid) -> ServiceResult<()> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(active) = sessions.iter_mut().find(|s| s.session.id == session_id) {
            if !active.session.paused {
                active.session.paused = true;
                active.writer.lock().await.file.flush().await.map_err(|e| {
                    AppError::IoError(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to flush on pause: {}", e)
                    ))
                })?;
                
                // Save checkpoint
                self.save_checkpoint(&active.session).await?;
                
                info!("Paused recording session {}", session_id);
                Ok(())
            } else {
                Err(AppError::ValidationError("Recording is already paused".to_string()))
            }
        } else {
            Err(AppError::NotFound(format!("Recording session {} not found", session_id)))
        }
    }
    
    async fn resume_recording(&self, session_id: Uuid) -> ServiceResult<()> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(active) = sessions.iter_mut().find(|s| s.session.id == session_id) {
            if active.session.paused {
                active.session.paused = false;
                info!("Resumed recording session {}", session_id);
                Ok(())
            } else {
                Err(AppError::ValidationError("Recording is not paused".to_string()))
            }
        } else {
            Err(AppError::NotFound(format!("Recording session {} not found", session_id)))
        }
    }
    
    async fn stop_recording(&self, session_id: Uuid) -> ServiceResult<RecordingMetadata> {
        // Find and remove session
        let (active_session, index) = {
            let mut sessions = self.sessions.write().await;
            
            if let Some(pos) = sessions.iter().position(|s| s.session.id == session_id) {
                let session = sessions.remove(pos);
                (session, pos)
            } else {
                return Err(AppError::NotFound(format!("Recording session {} not found", session_id)));
            }
        };
        
        // Close writer and get final file path
        let writer = Arc::try_unwrap(active_session.writer)
            .map_err(|_| AppError::ServiceError("Writer still in use".to_string()))?
            .into_inner();
        
        let final_path = writer.close().await?;
        
        // Calculate duration
        let duration_secs = (Utc::now() - active_session.started_at).num_seconds() as u64;
        
        // Get file size
        let metadata = tokio::fs::metadata(&final_path).await.map_err(|e| {
            AppError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to get file metadata: {}", e)
            ))
        })?;
        
        let recording = RecordingMetadata {
            id: active_session.session.id,
            meeting_id: active_session.session.meeting_id,
            user_id: Uuid::nil(), // TODO: Get from auth context
            device_name: "Default".to_string(),
            started_at: active_session.started_at,
            duration_secs,
            file_size_bytes: metadata.len(),
            file_path: final_path.to_string_lossy().to_string(),
            status: RecordingStatus::Completed,
            created_at: Utc::now(),
        };
        
        // Save to database
        sqlx::query!(
            r#"
            INSERT INTO recordings (
                id, meeting_id, user_id, device_name, started_at,
                duration_secs, file_size_bytes, file_path, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            recording.id,
            recording.meeting_id,
            recording.user_id,
            recording.device_name,
            recording.started_at,
            recording.duration_secs as i64,
            recording.file_size_bytes as i64,
            recording.file_path,
            "completed"
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        // Delete checkpoint
        sqlx::query!("DELETE FROM recording_sessions WHERE id = $1", session_id)
            .execute(&self.db_pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        info!("Stopped recording session {} ({} bytes, {} secs)", session_id, metadata.len(), duration_secs);
        Ok(recording)
    }
    
    async fn write_chunk(
        &self,
        session_id: Uuid,
        audio_data: Bytes,
        _timestamp: DateTime<Utc>,
    ) -> ServiceResult<u64> {
        let sessions = self.sessions.read().await;
        
        if let Some(active) = sessions.iter().find(|s| s.session.id == session_id) {
            if active.session.paused {
                return Err(AppError::ValidationError("Cannot write to paused recording".to_string()));
            }
            
            let mut writer = active.writer.lock().await;
            let bytes_written = writer.write_chunk(&audio_data).await?;
            
            // Update chunk count
            let session = &mut active.session.clone();
            session.chunks_written += 1;
            
            // Checkpoint periodically
            active.chunks_since_checkpoint += 1;
            if active.chunks_since_checkpoint >= CHECKPOINT_INTERVAL {
                self.save_checkpoint(session).await?;
                active.chunks_since_checkpoint = 0;
                active.last_checkpoint = Utc::now();
            }
            
            debug!("Wrote chunk to session {} ({} bytes total)", session_id, bytes_written);
            Ok(bytes_written)
        } else {
            Err(AppError::NotFound(format!("Recording session {} not found", session_id)))
        }
    }
    
    async fn get_session_status(&self, session_id: Uuid) -> ServiceResult<RecordingSession> {
        let sessions = self.sessions.read().await;
        
        if let Some(active) = sessions.iter().find(|s| s.session.id == session_id) {
            Ok(active.session.clone())
        } else {
            Err(AppError::NotFound(format!("Recording session {} not found", session_id)))
        }
    }
    
    async fn recover_session(&self, session_id: Uuid) -> ServiceResult<RecordingSession> {
        // Try to load from checkpoint
        if let Some(session) = self.load_checkpoint(session_id).await? {
            // Recreate writer
            let max_file_size = session.config.max_file_size_mb
                .map(|mb| mb * 1024 * 1024)
                .unwrap_or(DEFAULT_MAX_CHUNK_SIZE_BYTES);
            
            let writer = StreamingWriter::new(
                PathBuf::from(&session.config.storage_path),
                session_id,
                &session.config,
                max_file_size,
            ).await?;
            
            let active_session = ActiveSession {
                session: session.clone(),
                writer: Arc::new(Mutex::new(writer)),
                started_at: session.started_at,
                last_checkpoint: Utc::now(),
                chunks_since_checkpoint: 0,
            };
            
            // Add back to active sessions
            {
                let mut sessions = self.sessions.write().await;
                sessions.push(active_session);
            }
            
            info!("Recovered recording session {} from checkpoint", session_id);
            Ok(session)
        } else {
            Err(AppError::NotFound(format!("No checkpoint found for session {}", session_id)))
        }
    }
    
    async fn get_recording(&self, recording_id: Uuid) -> ServiceResult<RecordingMetadata> {
        let record = sqlx::query_as!(
            RecordingMetadataRow,
            r#"
            SELECT 
                id, meeting_id, user_id, device_name, started_at,
                duration_secs, file_size_bytes, file_path, status, created_at
            FROM recordings
            WHERE id = $1
            "#,
            recording_id
        )
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        match record {
            Some(row) => Ok(row.into()),
            None => Err(AppError::NotFound(format!("Recording {} not found", recording_id))),
        }
    }
    
    async fn list_recordings(&self, meeting_id: Uuid) -> ServiceResult<Vec<RecordingMetadata>> {
        let records = sqlx::query_as!(
            RecordingMetadataRow,
            r#"
            SELECT 
                id, meeting_id, user_id, device_name, started_at,
                duration_secs, file_size_bytes, file_path, status, created_at
            FROM recordings
            WHERE meeting_id = $1
            ORDER BY created_at DESC
            "#,
            meeting_id
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(records.into_iter().map(|r| r.into()).collect())
    }
    
    async fn delete_recording(&self, recording_id: Uuid) -> ServiceResult<()> {
        // Get recording to delete file
        let recording = self.get_recording(recording_id).await?;
        
        // Delete file
        if let Err(e) = tokio::fs::remove_file(&recording.file_path).await {
            warn!("Failed to delete recording file {}: {}", recording.file_path, e);
            // Continue anyway - database cleanup is more important
        }
        
        // Delete from database
        sqlx::query!("DELETE FROM recordings WHERE id = $1", recording_id)
            .execute(&self.db_pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        info!("Deleted recording {}", recording_id);
        Ok(())
    }
}

/// Database row mapping helper
#[derive(sqlx::FromRow)]
struct RecordingMetadataRow {
    id: Uuid,
    meeting_id: Uuid,
    user_id: Uuid,
    device_name: String,
    started_at: DateTime<Utc>,
    duration_secs: i64,
    file_size_bytes: i64,
    file_path: String,
    status: String,
    created_at: DateTime<Utc>,
}

impl From<RecordingMetadataRow> for RecordingMetadata {
    fn from(row: RecordingMetadataRow) -> Self {
        Self {
            id: row.id,
            meeting_id: row.meeting_id,
            user_id: row.user_id,
            device_name: row.device_name,
            started_at: row.started_at,
            duration_secs: row.duration_secs as u64,
            file_size_bytes: row.file_size_bytes as u64,
            file_path: row.file_path,
            status: match row.status.as_str() {
                "recording" => RecordingStatus::Recording,
                "paused" => RecordingStatus::Paused,
                "completed" => RecordingStatus::Completed,
                "failed" => RecordingStatus::Failed("Unknown".to_string()),
                _ => RecordingStatus::Completed,
            },
            created_at: row.created_at,
        }
    }
}

// Database schema migration SQL (to be added to migrations/)
// CREATE TABLE IF NOT EXISTS recording_sessions (
//     id UUID PRIMARY KEY,
//     meeting_id UUID NOT NULL,
//     config_json JSONB NOT NULL,
//     started_at TIMESTAMPTZ NOT NULL,
//     paused BOOLEAN NOT NULL DEFAULT FALSE,
//     chunks_written BIGINT NOT NULL DEFAULT 0,
//     current_file_path TEXT NOT NULL,
//     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
//     updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
// );
//
// CREATE TABLE IF NOT EXISTS recordings (
//     id UUID PRIMARY KEY,
//     meeting_id UUID NOT NULL,
//     user_id UUID NOT NULL,
//     device_name TEXT NOT NULL,
//     started_at TIMESTAMPTZ NOT NULL,
//     duration_secs BIGINT NOT NULL,
//     file_size_bytes BIGINT NOT NULL,
//     file_path TEXT NOT NULL,
//     status TEXT NOT NULL,
//     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
// );
//
// CREATE INDEX idx_recordings_meeting_id ON recordings(meeting_id);
// CREATE INDEX idx_sessions_meeting_id ON recording_sessions(meeting_id);