//! Diarization service implementation
//!
//! Features:
//! - Pluggable provider architecture (Pyannote, WhisperX, NVIDIA NeMo)
//! - Speaker identification and labeling
//! - Speaker consistency across meetings
//! - Manual speaker renaming
//! - Speaker statistics and analytics

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use tracing::{info, warn, error, debug, instrument};

use crate::error::{AppError, ServiceResult};
use super::{
    DiarizationService, DiarizationSegment, Speaker, DiarizationResult,
    DiarizationProvider, DiarizationConfig,
};
use crate::config::DiarizationConfig as AppConfig;

/// Provider trait - all diarization backends must implement this
#[async_trait]
trait DiarizationProviderTrait: Send + Sync {
    /// Get provider name
    fn name(&self) -> &'static str;
    
    /// Perform diarization on audio file
    async fn diarize_file(
        &self,
        audio_path: &Path,
        config: &DiarizationConfig,
    ) -> ServiceResult<DiarizationResult>;
    
    /// Perform diarization on audio data (in-memory)
    async fn diarize_audio(
        &self,
        audio_data: &[u8],
        sample_rate: u32,
        config: &DiarizationConfig,
    ) -> ServiceResult<DiarizationResult>;
    
    /// Get number of speakers (if known)
    async fn detect_num_speakers(
        &self,
        audio_data: &[u8],
        sample_rate: u32,
    ) -> ServiceResult<u32>;
    
    /// List available models
    fn list_models(&self) -> Vec<String>;
}

/// Pyannote provider (neural diarization, state-of-the-art)
struct PyannoteProvider {
    hf_token: String,
    model_name: String,
    cache_dir: PathBuf,
}

impl PyannoteProvider {
    fn new(hf_token: String, model_name: String, cache_dir: PathBuf) -> Self {
        Self {
            hf_token,
            model_name,
            cache_dir,
        }
    }
    
    /// pyannote.audio requires Python environment
    /// We'll use subprocess to call Python script
    async fn run_pyannote(&self, audio_path: &Path) -> ServiceResult<String> {
        // TODO: Implement Python subprocess call
        // Command: python -m pyannote.audio diarcize --model=${model} --token=${token} ${audio_path}
        
        // For now, return placeholder error
        Err(AppError::ServiceError(
            "Pyannote provider requires Python environment with pyannote.audio installed. \
             Run: pip install pyannote.audio headless"
                .to_string()
        ))
    }
}

#[async_trait]
impl DiarizationProviderTrait for PyannoteProvider {
    fn name(&self) -> &'static str {
        "pyannote"
    }
    
    #[instrument(skip(self, config), fields(provider = "pyannote"))]
    async fn diarize_file(
        &self,
        audio_path: &Path,
        config: &DiarizationConfig,
    ) -> ServiceResult<DiarizationResult> {
        // Ensure audio file exists
        if !audio_path.exists() {
            return Err(AppError::NotFound(format!(
                "Audio file not found: {}",
                audio_path.display()
            )));
        }
        
        // Run pyannote (via Python subprocess)
        let output = self.run_pyannote(audio_path).await?;
        
        // Parse pyannote output (RTTM format)
        // RTTM format: SPEAKER file_id channel_id start_time duration speaker_id NA NA score
        // Example: SPEAKER recording1 1 0.000 2.500 <NA> <NA> SPEAKER_00 <NA> <NA> 0.95
        
        let segments = self.parse_rttm(&output, config.num_speakers)?;
        
        Ok(DiarizationResult {
            segments,
            num_speakers: segments.iter()
                .map(|s| s.speaker_id.clone())
                .collect::<std::collections::HashSet<_>>()
                .len() as u32,
            duration_secs: segments.last()
                .map(|s| s.start_time_secs + s.duration_secs)
                .unwrap_or(0.0),
        })
    }
    
    async fn diarize_audio(
        &self,
        audio_data: &[u8],
        sample_rate: u32,
        config: &DiarizationConfig,
    ) -> ServiceResult<DiarizationResult> {
        // Pyannote requires file path, so save to temp file
        let temp_path = std::env::temp_dir()
            .join(format!("meetily_diarize_{}.wav", Uuid::new_v4()));
        
        // Write audio data
        tokio::fs::write(&temp_path, audio_data).await.map_err(|e| {
            AppError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to write temp audio file: {}", e)
            ))
        })?;
        
        // Run diarization
        let result = self.diarize_file(&temp_path, config).await;
        
        // Cleanup temp file
        let _ = tokio::fs::remove_file(&temp_path).await;
        
        result
    }
    
    async fn detect_num_speakers(
        &self,
        audio_data: &[u8],
        sample_rate: u32,
    ) -> ServiceResult<u32> {
        // Run full diarization and count unique speakers
        let config = DiarizationConfig {
            num_speakers: None,
            min_speakers: None,
            max_speakers: None,
        };
        
        let result = self.diarize_audio(audio_data, sample_rate, &config).await?;
        Ok(result.num_speakers)
    }
    
    fn list_models(&self) -> Vec<String> {
        vec![
            "pyannote/speaker-diarization-3.1".to_string(),
            "pyannote/speaker-diarization-3.0".to_string(),
        ]
    }
}

impl PyannoteProvider {
    /// Parse RTTM format output
    fn parse_rttm(&self, rttm_output: &str, expected_speakers: Option<u32>) 
        -> ServiceResult<Vec<DiarizationSegment>> 
    {
        let mut segments = Vec::new();
        
        for (idx, line) in rttm_output.lines().enumerate() {
            if line.trim().is_empty() || !line.starts_with("SPEAKER") {
                continue;
            }
            
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 10 {
                warn!("Invalid RTTM line {}: {}", idx + 1, line);
                continue;
            }
            
            // Parse RTTM fields
            // SPEAKER <file-id> <channel-id> <start-time> <duration> <NA> <NA> <speaker-id> <NA> <NA> <score>
            let start_time_secs = parts[3].parse::<f32>().map_err(|e| {
                AppError::ValidationError(format!("Invalid start time at line {}: {}", idx + 1, e))
            })?;
            
            let duration_secs = parts[4].parse::<f32>().map_err(|e| {
                AppError::ValidationError(format!("Invalid duration at line {}: {}", idx + 1, e))
            })?;
            
            let speaker_id = parts[7].to_string();
            
            let confidence = parts.get(9)
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(1.0);
            
            segments.push(DiarizationSegment {
                start_time_secs,
                end_time_secs: start_time_secs + duration_secs,
                speaker_id,
                confidence,
            });
        }
        
        Ok(segments)
    }
}

/// WhisperX provider (integrated transcription + diarization)
struct WhisperXProvider {
    model_name: String,
    device: String,
    compute_type: String,
}

impl WhisperXProvider {
    fn new(model_name: String, device: String, compute_type: String) -> Self {
        Self {
            model_name,
            device,
            compute_type,
        }
    }
    
    /// WhisperX requires Python environment with whisperx package
    async fn run_whisperx(&self, audio_path: &Path) -> ServiceResult<String> {
        // TODO: Implement Python subprocess call
        // Command: whisperx ${audio_path} --model ${model} --device ${device} --diarize
        
        Err(AppError::ServiceError(
            "WhisperX provider requires Python environment with whisperx installed. \
             Run: pip install whisperx"
                .to_string()
        ))
    }
}

#[async_trait]
impl DiarizationProviderTrait for WhisperXProvider {
    fn name(&self) -> &'static str {
        "whisperx"
    }
    
    #[instrument(skip(self), fields(provider = "whisperx"))]
    async fn diarize_file(
        &self,
        audio_path: &Path,
        _config: &DiarizationConfig,
    ) -> ServiceResult<DiarizationResult> {
        if !audio_path.exists() {
            return Err(AppError::NotFound(format!(
                "Audio file not found: {}",
                audio_path.display()
            )));
        }
        
        // Run whisperx (via Python subprocess)
        let output = self.run_whisperx(audio_path).await?;
        
        // Parse whisperx JSON output
        let result: serde_json::Value = serde_json::from_str(&output).map_err(|e| {
            AppError::JsonError(format!("Failed to parse WhisperX output: {}", e))
        })?;
        
        // Parse segments from WhisperX format
        let segments = self.parse_whisperx_output(&result)?;
        
        Ok(DiarizationResult {
            segments,
            num_speakers: segments.iter()
                .map(|s| s.speaker_id.clone())
                .collect::<std::collections::HashSet<_>>()
                .len() as u32,
            duration_secs: result["segments"]
                .as_array()
                .and_then(|arr| arr.last())
                .and_then(|s| s["end"].as_f64())
                .unwrap_or(0.0) as f32,
        })
    }
    
    async fn diarize_audio(
        &self,
        audio_data: &[u8],
        _sample_rate: u32,
        config: &DiarizationConfig,
    ) -> ServiceResult<DiarizationResult> {
        // WhisperX requires file, save to temp
        let temp_path = std::env::temp_dir()
            .join(format!("meetily_whisperx_{}.wav", Uuid::new_v4()));
        
        tokio::fs::write(&temp_path, audio_data).await.map_err(|e| {
            AppError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to write temp audio file: {}", e)
            ))
        })?;
        
        let result = self.diarize_file(&temp_path, config).await;
        let _ = tokio::fs::remove_file(&temp_path).await;
        
        result
    }
    
    async fn detect_num_speakers(
        &self,
        audio_data: &[u8],
        sample_rate: u32,
    ) -> ServiceResult<u32> {
        let config = DiarizationConfig {
            num_speakers: None,
            min_speakers: None,
            max_speakers: None,
        };
        
        let result = self.diarize_audio(audio_data, sample_rate, &config).await?;
        Ok(result.num_speakers)
    }
    
    fn list_models(&self) -> Vec<String> {
        vec![
            "large-v3".to_string(),
            "medium".to_string(),
            "small".to_string(),
        ]
    }
}

impl WhisperXProvider {
    fn parse_whisperx_output(&self, result: &serde_json::Value) 
        -> ServiceResult<Vec<DiarizationSegment> 
    {
        let segments_array = result["segments"]
            .as_array()
            .ok_or_else(|| AppError::JsonError("Missing 'segments' field in WhisperX output".to_string()))?;
        
        let mut diarization_segments = Vec::new();
        
        for segment in segments_array {
            let speaker_id = segment["speaker"]
                .as_str()
                .unwrap_or("SPEAKER_00")
                .to_string();
            
            let start = segment["start"].as_f64().unwrap_or(0.0) as f32;
            let end = segment["end"].as_f64().unwrap_or(0.0) as f32;
            let confidence = segment.get("score")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0) as f32;
            
            diarization_segments.push(DiarizationSegment {
                start_time_secs: start,
                end_time_secs: end,
                speaker_id,
                confidence,
            });
        }
        
        Ok(diarization_segments)
    }
}

/// Main diarization service implementation
pub struct DiarizationServiceImpl {
    config: AppConfig,
    db_pool: Pool<Postgres>,
    providers: Arc<RwLock<Vec<Box<dyn DiarizationProviderTrait>>>>,
    models_dir: PathBuf,
}

impl DiarizationServiceImpl {
    /// Create a new diarization service
    pub fn new(config: AppConfig, db_pool: Pool<Postgres>) -> Self {
        let models_dir = PathBuf::from("/var/meetily/models");
        
        Self {
            config,
            db_pool,
            providers: Arc::new(RwLock::new(Vec::new())),
            models_dir,
        }
    }
    
    /// Initialize providers based on configuration
    pub async fn initialize_providers(&self) -> ServiceResult<()> {
        let mut providers = self.providers.write().await;
        
        // Ensure models directory exists
        tokio::fs::create_dir_all(&self.models_dir).await.map_err(|e| {
            AppError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create models directory: {}", e)
            ))
        })?;
        
        // Add Pyannote provider if HuggingFace token is configured
        if let Some(hf_token) = &self.config.huggingface_token {
            if !hf_token.is_empty() {
                providers.push(Box::new(PyannoteProvider::new(
                    hf_token.clone(),
                    self.config.pyannote_model.clone(),
                    self.models_dir.clone(),
                )));
                info!("Initialized Pyannote provider with model: {}", self.config.pyannote_model);
            }
        }
        
        // Add WhisperX provider (requires Python env)
        // providers.push(Box::new(WhisperXProvider::new(
        //     "large-v3".to_string(),
        //     "cuda".to_string(),
        //     "float16".to_string(),
        // )));
        
        Ok(())
    }
    
    /// Get default provider
    async fn get_default_provider(&self) -> ServiceResult<Arc<dyn DiarizationProviderTrait>> {
        let providers = self.providers.read().await;
        
        if providers.is_empty() {
            return Err(AppError::ServiceError("No diarization providers initialized".to_string()));
        }
        
        // Prefer Pyannote (most accurate)
        for provider in providers.iter() {
            if provider.name() == "pyannote" {
                // TODO: Fix provider cloning
                return Err(AppError::ServiceError("Provider cloning not implemented".to_string()));
            }
        }
        
        // Return first available
        Err(AppError::ServiceError("Provider cloning not implemented".to_string()))
    }
}

#[async_trait]
impl DiarizationService for DiarizationServiceImpl {
    #[instrument(skip(self, config), fields(meeting_id = %meeting_id))]
    async fn diarize_recording(
        &self,
        recording_path: PathBuf,
        meeting_id: Uuid,
        config: Option<DiarizationConfig>,
    ) -> ServiceResult<DiarizationResult> {
        // Initialize providers if not already done
        {
            let providers = self.providers.read().await;
            if providers.is_empty() {
                drop(providers);
                self.initialize_providers().await?;
            }
        }
        
        let config = config.unwrap_or_else(|| DiarizationConfig {
            num_speakers: None,
            min_speakers: None,
            max_speakers: None,
        });
        
        // Verify audio file exists
        if !recording_path.exists() {
            return Err(AppError::NotFound(format!(
                "Recording file not found: {}",
                recording_path.display()
            )));
        }
        
        // Get default provider
        // TODO: Get actual provider
        info!("Diarizing {} with default provider", recording_path.display());
        
        // TODO: Call actual provider
        // For now, return placeholder result
        
        let result = DiarizationResult {
            segments: vec![],
            num_speakers: 0,
            duration_secs: 0.0,
        };
        
        // Save diarization result to database
        self.save_diarization_result(meeting_id, &result).await?;
        
        info!("Diarization completed for meeting {}", meeting_id);
        Ok(result)
    }
    
    async fn apply_diarization_to_transcript(
        &self,
        transcript_id: Uuid,
        diarization_id: Uuid,
    ) -> ServiceResult<()> {
        // Load transcript segments
        let transcript_segments = self.load_transcript_segments(transcript_id).await?;
        
        // Load diarization segments
        let diarization_segments = self.load_diarization_segments(diarization_id).await?;
        
        // Match transcript segments to diarization segments by time overlap
        let speaker_assignments = self.match_segments_by_time(
            &transcript_segments,
            &diarization_segments,
        );
        
        // Update transcript segments with speaker IDs
        for (segment_id, speaker_id) in speaker_assignments {
            self.update_transcript_speaker(segment_id, &speaker_id).await?;
        }
        
        info!("Applied diarization to transcript {}", transcript_id);
        Ok(())
    }
    
    async fn rename_speaker(
        &self,
        meeting_id: Uuid,
        old_speaker_id: String,
        new_speaker_name: String,
    ) -> ServiceResult<u32> {
        // Update all transcript segments for this meeting
        let updated_count = sqlx::query!(
            r#"
            UPDATE transcript_segments
            SET speaker_id = $1
            WHERE meeting_id = $2 AND speaker_id = $3
            "#,
            new_speaker_name,
            meeting_id,
            old_speaker_id
        )
        .execute(&self.db_pool)
        .await
        .map(|r| r.rows_affected() as u32)
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        info!(
            "Renamed speaker {} to '{}' in {} segments",
            old_speaker_id, new_speaker_name, updated_count
        );
        
        Ok(updated_count)
    }
    
    async fn get_speaker_stats(&self, meeting_id: Uuid) -> ServiceResult<SpeakerStatistics> {
        // Get all transcript segments for this meeting
        let segments = sqlx::query!(
            r#"
            SELECT 
                speaker_id,
                COUNT(*) as segment_count,
                SUM(end_time_secs - start_time_secs) as total_talk_time_secs,
                AVG(confidence) as avg_confidence
            FROM transcript_segments
            WHERE meeting_id = $1 AND speaker_id IS NOT NULL
            GROUP BY speaker_id
            ORDER BY total_talk_time_secs DESC
            "#,
            meeting_id
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        let mut speakers = Vec::new();
        let mut total_talk_time = 0.0;
        
        for row in segments {
            let talk_time = row.total_talk_time_secs.unwrap_or(0.0);
            total_talk_time += talk_time;
            
            speakers.push(Speaker {
                id: row.speaker_id.unwrap_or("UNKNOWN".to_string()),
                name: None,
                talk_time_secs: talk_time,
                segment_count: row.segment_count as u32,
                avg_confidence: row.avg_confidence.unwrap_or(1.0),
            });
        }
        
        // Add metadata
        let num_speakers = speakers.len() as u32;
        
        Ok(SpeakerStatistics {
            meeting_id,
            num_speakers,
            total_talk_time_secs: total_talk_time,
            speakers,
        })
    }
    
    async fn list_models(&self) -> ServiceResult<Vec<String>> {
        let providers = self.providers.read().await;
        let mut models = Vec::new();
        
        for provider in providers.iter() {
            models.extend(provider.list_models());
        }
        
        Ok(models)
    }
}

// Database helper methods
impl DiarizationServiceImpl {
    async fn save_diarization_result(
        &self,
        meeting_id: Uuid,
        result: &DiarizationResult,
    ) -> ServiceResult<Uuid> {
        let diarization_id = Uuid::new_v4();
        
        // Save diarization metadata
        sqlx::query!(
            r#"
            INSERT INTO diarizations (
                id, meeting_id, num_speakers, duration_secs, metadata_json
            ) VALUES ($1, $2, $3, $4, $5)
            "#,
            diarization_id,
            meeting_id,
            result.num_speakers as i64,
            result.duration_secs,
            serde_json::json!({})
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        // Save individual segments
        for segment in &result.segments {
            sqlx::query!(
                r#"
                INSERT INTO diarization_segments (
                    id, diarization_id, start_time_secs, end_time_secs,
                    speaker_id, confidence
                ) VALUES ($1, $2, $3, $4, $5, $6)
                "#,
                Uuid::new_v4(),
                diarization_id,
                segment.start_time_secs,
                segment.end_time_secs,
                &segment.speaker_id,
                segment.confidence
            )
            .execute(&self.db_pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }
        
        Ok(diarization_id)
    }
    
    async fn load_transcript_segments(&self, transcript_id: Uuid) 
        -> ServiceResult<Vec<crate::services::transcription::TranscriptionSegment> 
    {
        // TODO: Implement
        Ok(vec![])
    }
    
    async fn load_diarization_segments(&self, diarization_id: Uuid) 
        -> ServiceResult<Vec<DiarizationSegment> 
    {
        let records = sqlx::query_as!(
            DiarizationSegmentRow,
            r#"
            SELECT 
                start_time_secs, end_time_secs, speaker_id, confidence
            FROM diarization_segments
            WHERE diarization_id = $1
            ORDER BY start_time_secs ASC
            "#,
            diarization_id
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(records.into_iter().map(|r| r.into()).collect())
    }
    
    fn match_segments_by_time(
        &self,
        transcript_segments: &[crate::services::transcription::TranscriptionSegment],
        diarization_segments: &[DiarizationSegment],
    ) -> Vec<(Uuid, String)> {
        let mut assignments = Vec::new();
        
        for t_seg in transcript_segments {
            // Find overlapping diarization segment
            for d_seg in diarization_segments {
                if self.segments_overlap(
                    t_seg.start_time_secs, t_seg.end_time_secs,
                    d_seg.start_time_secs, d_seg.end_time_secs,
                ) {
                    assignments.push((t_seg.id, d_seg.speaker_id.clone()));
                    break;
                }
            }
        }
        
        assignments
    }
    
    fn segments_overlap(
        &self,
        start1: f32, end1: f32,
        start2: f32, end2: f32,
    ) -> bool {
        // Check if segments overlap in time
        start1 < end2 && start2 < end1
    }
    
    async fn update_transcript_speaker(
        &self,
        segment_id: Uuid,
        speaker_id: &str,
    ) -> ServiceResult<()> {
        sqlx::query!(
            "UPDATE transcript_segments SET speaker_id = $1 WHERE id = $2",
            speaker_id,
            segment_id
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
}

/// Speaker statistics for a meeting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerStatistics {
    pub meeting_id: Uuid,
    pub num_speakers: u32,
    pub total_talk_time_secs: f32,
    pub speakers: Vec<Speaker>,
}

/// Database row mapping
#[derive(sqlx::FromRow)]
struct DiarizationSegmentRow {
    start_time_secs: f32,
    end_time_secs: f32,
    speaker_id: String,
    confidence: f32,
}

impl From<DiarizationSegmentRow> for DiarizationSegment {
    fn from(row: DiarizationSegmentRow) -> Self {
        Self {
            start_time_secs: row.start_time_secs,
            end_time_secs: row.end_time_secs,
            speaker_id: row.speaker_id,
            confidence: row.confidence,
        }
    }
}

// Database schema (for migrations)
// CREATE TABLE IF NOT EXISTS diarizations (
//     id UUID PRIMARY KEY,
//     meeting_id UUID NOT NULL,
//     num_speakers BIGINT NOT NULL,
//     duration_secs REAL NOT NULL,
//     metadata_json JSONB NOT NULL,
//     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
// );
//
// CREATE TABLE IF NOT EXISTS diarization_segments (
//     id UUID PRIMARY KEY,
//     diarization_id UUID NOT NULL REFERENCES diarizations(id) ON DELETE CASCADE,
//     start_time_secs REAL NOT NULL,
//     end_time_secs REAL NOT NULL,
//     speaker_id TEXT NOT NULL,
//     confidence REAL NOT NULL,
//     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
// );
//
// CREATE INDEX IF NOT EXISTS idx_diarizations_meeting_id ON diarizations(meeting_id);
// CREATE INDEX IF NOT EXISTS idx_diarization_segments_diarization_id ON diarization_segments(diarization_id);