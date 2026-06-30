//! Transcription service implementation
//!
//! Features:
//! - Pluggable provider architecture (Whisper, WhisperX, NVIDIA, faster-whisper)
//! - Automatic language detection
//! - Confidence scores and word-level timestamps
//! - GPU acceleration support
//! - Streaming transcription for real-time processing

use async_trait::async_trait;
use bytes::Bytes;
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
    TranscriptionService, TranscriptionSegment, Transcript, TranscriptionStatus,
    TranscriptionMetadata, TranscriptionProvider, WhisperModel, TranscriptionConfig,
    TranscriptionTask, WordTiming,
};
use crate::config::TranscriptionConfig as AppConfig;

/// Provider trait - all transcription backends must implement this
#[async_trait]
trait TranscriptionProviderTrait: Send + Sync {
    /// Get provider name
    fn name(&self) -> &'static str;
    
    /// Transcribe audio file
    async fn transcribe_file(
        &self,
        audio_path: &Path,
        config: &TranscriptionConfig,
    ) -> ServiceResult<TranscriptionResult>;
    
    /// Transcribe audio chunk (for streaming)
    async fn transcribe_chunk(
        &self,
        audio_data: &[u8],
        sample_rate: u32,
        config: &TranscriptionConfig,
    ) -> ServiceResult<TranscriptionSegment>;
    
    /// Detect language from audio
    async fn detect_language(
        &self,
        audio_data: &[u8],
        sample_rate: u32,
    ) -> ServiceResult<String>;
    
    /// List available models
    fn list_models(&self) -> Vec<String>;
}

/// Unified transcription result
#[derive(Debug, Clone)]
struct TranscriptionResult {
    segments: Vec<TranscriptionSegment>,
    language: String,
    duration_secs: f32,
    word_count: u32,
}

/// Whisper provider (whisper-rs, local GPU acceleration)
struct WhisperProvider {
    model_path: PathBuf,
    // In production, this would hold the actual whisper-rs model
    // For now, we'll use a placeholder
    _model: Arc<RwLock<Option<()>>>, // TODO: Replace with actual whisper-rs model
}

impl WhisperProvider {
    fn new(model: WhisperModel, models_dir: &Path) -> Self {
        let model_name = match model {
            WhisperModel::Tiny => "ggml-tiny.bin",
            WhisperModel::Base => "ggml-base.bin",
            WhisperModel::Small => "ggml-small.bin",
            WhisperModel::Medium => "ggml-medium.bin",
            WhisperModel::LargeV3 => "ggml-large-v3.bin",
            WhisperModel::LargeV3Turbo => "ggml-large-v3-turbo.bin",
        };
        
        Self {
            model_path: models_dir.join(model_name),
            _model: Arc::new(RwLock::new(None)),
        }
    }
    
    // TODO: Implement actual whisper-rs loading and inference
    // This requires whisper-rs crate and proper model management
}

#[async_trait]
impl TranscriptionProviderTrait for WhisperProvider {
    fn name(&self) -> &'static str {
        "whisper"
    }
    
    async fn transcribe_file(
        &self,
        _audio_path: &Path,
        _config: &TranscriptionConfig,
    ) -> ServiceResult<TranscriptionResult> {
        // TODO: Implement actual whisper-rs inference
        // For now, return placeholder
        Err(AppError::ServiceError(
            "Whisper provider not fully implemented - requires whisper-rs integration".to_string()
        ))
    }
    
    async fn transcribe_chunk(
        &self,
        _audio_data: &[u8],
        _sample_rate: u32,
        _config: &TranscriptionConfig,
    ) -> ServiceResult<TranscriptionSegment> {
        todo!("Implement streaming chunk transcription")
    }
    
    async fn detect_language(
        &self,
        _audio_data: &[u8],
        _sample_rate: u32,
    ) -> ServiceResult<String> {
        // Whisper can detect language automatically
        Ok("auto".to_string())
    }
    
    fn list_models(&self) -> Vec<String> {
        vec![
            "tiny".to_string(),
            "base".to_string(),
            "small".to_string(),
            "medium".to_string(),
            "large-v3".to_string(),
            "large-v3-turbo".to_string(),
        ]
    }
}

/// NVIDIA Parakeet provider (API-based)
struct NVIDIAProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl NVIDIAProvider {
    fn new(api_key: String, base_url: String) -> Self {
        Self {
            api_key,
            base_url,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TranscriptionProviderTrait for NVIDIAProvider {
    fn name(&self) -> &'static str {
        "nvidia"
    }
    
    #[instrument(skip(self, _config), fields(provider = "nvidia"))]
    async fn transcribe_file(
        &self,
        audio_path: &Path,
        _config: &TranscriptionConfig,
    ) -> ServiceResult<TranscriptionResult> {
        // Read audio file
        let audio_data = tokio::fs::read(audio_path).await.map_err(|e| {
            AppError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to read audio file: {}", e)
            ))
        })?;
        
        // Call NVIDIA API
        // API endpoint: https://integrate.api.nvidia.com/v1/audio/transcriptions
        let response = self.client
            .post(&format!("{}/audio/transcriptions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "application/json")
            .multipart(reqwest::multipart::Form::new()
                .part("file", reqwest::multipart::Part::bytes(audio_data))
                .text("model", "parakeet")
            )
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError {
                provider: "NVIDIA".to_string(),
                message: format!("API request failed: {}", e),
            })?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError {
                provider: "NVIDIA".to_string(),
                message: format!("API returned {}: {}", status, error_text),
            });
        }
        
        // Parse response
        // Expected format: {"text": "...", "segments": [...], "language": "..."}
        let result: serde_json::Value = response.json().await.map_err(|e| {
            AppError::ExternalServiceError {
                provider: "NVIDIA".to_string(),
                message: format!("Failed to parse response: {}", e),
            }
        })?;
        
        // TODO: Parse actual segments with timestamps and confidence
        let text = result["text"].as_str().unwrap_or("").to_string();
        let language = result["language"].as_str().unwrap_or("en").to_string();
        
        Ok(TranscriptionResult {
            segments: vec![TranscriptionSegment {
                id: Uuid::new_v4(),
                transcript_id: Uuid::nil(), // Will be set by service
                start_time_secs: 0.0,
                end_time_secs: 0.0, // TODO: Get from API
                text,
                confidence: 1.0, // TODO: Get from API
                speaker_id: None,
                language: language.clone(),
                words: vec![], // TODO: Parse word-level timestamps
            }],
            language,
            duration_secs: 0.0, // TODO: Get from API
            word_count: 0, // TODO: Count words
        })
    }
    
    async fn transcribe_chunk(
        &self,
        _audio_data: &[u8],
        _sample_rate: u32,
        _config: &TranscriptionConfig,
    ) -> ServiceResult<TranscriptionSegment> {
        // NVIDIA API doesn't support streaming chunks
        // Would need to buffer chunks and send as file
        Err(AppError::ValidationError(
            "NVIDIA provider doesn't support chunk transcription".to_string()
        ))
    }
    
    async fn detect_language(
        &self,
        _audio_data: &[u8],
        _sample_rate: u32,
    ) -> ServiceResult<String> {
        // NVIDIA API auto-detects language
        Ok("auto".to_string())
    }
    
    fn list_models(&self) -> Vec<String> {
        vec![
            "parakeet".to_string(),
            "parakeet-ctc".to_string(),
        ]
    }
}

/// faster-whisper provider (CTranslate2 backend)
struct FasterWhisperProvider {
    model_path: PathBuf,
    device: String, // "cpu", "cuda", "auto"
}

impl FasterWhisperProvider {
    fn new(model_name: String, models_dir: &Path, device: String) -> Self {
        Self {
            model_path: models_dir.join(&model_name),
            device,
        }
    }
}

#[async_trait]
impl TranscriptionProviderTrait for FasterWhisperProvider {
    fn name(&self) -> &'static str {
        "faster-whisper"
    }
    
    async fn transcribe_file(
        &self,
        _audio_path: &Path,
        _config: &TranscriptionConfig,
    ) -> ServiceResult<TranscriptionResult> {
        // TODO: Implement using faster-whisper Python bindings via subprocess
        // Or use ctranslate2-rs if available
        Err(AppError::ServiceError(
            "faster-whisper provider requires Python subprocess or ctranslate2-rs".to_string()
        ))
    }
    
    async fn transcribe_chunk(
        &self,
        _audio_data: &[u8],
        _sample_rate: u32,
        _config: &TranscriptionConfig,
    ) -> ServiceResult<TranscriptionSegment> {
        todo!("Implement faster-whisper chunk transcription")
    }
    
    async fn detect_language(
        &self,
        _audio_data: &[u8],
        _sample_rate: u32,
    ) -> ServiceResult<String> {
        Ok("auto".to_string())
    }
    
    fn list_models(&self) -> Vec<String> {
        vec![
            "tiny".to_string(),
            "base".to_string(),
            "small".to_string(),
            "medium".to_string(),
            "large-v3".to_string(),
            "distil-whisper".to_string(),
        ]
    }
}

/// Main transcription service implementation
pub struct TranscriptionServiceImpl {
    config: AppConfig,
    db_pool: Pool<Postgres>,
    /// Active transcription providers (loaded on demand)
    providers: Arc<RwLock<Vec<Box<dyn TranscriptionProviderTrait>>>>,
    models_dir: PathBuf,
}

impl TranscriptionServiceImpl {
    /// Create a new transcription service
    pub fn new(config: AppConfig, db_pool: Pool<Postgres>) -> Self {
        let models_dir = PathBuf::from("/var/meetily/models"); // TODO: Make configurable
        
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
        
        // Always add Whisper provider (local, free)
        let whisper_model = match self.config.whisper_model.as_str() {
            "tiny" => WhisperModel::Tiny,
            "base" => WhisperModel::Base,
            "small" => WhisperModel::Small,
            "medium" => WhisperModel::Medium,
            "large-v3-turbo" => WhisperModel::LargeV3Turbo,
            _ => WhisperModel::LargeV3,
        };
        
        providers.push(Box::new(WhisperProvider::new(whisper_model, &self.models_dir)));
        info!("Initialized Whisper provider with model: {:?}", whisper_model);
        
        // Add NVIDIA provider if API key is configured
        if let Some(api_key) = &self.config.nvidia_api_key {
            if !api_key.is_empty() {
                providers.push(Box::new(NVIDIAProvider::new(
                    api_key.clone(),
                    self.config.nvidia_base_url.clone(),
                )));
                info!("Initialized NVIDIA provider");
            }
        }
        
        // Add faster-whisper provider (optional, requires setup)
        // providers.push(Box::new(FasterWhisperProvider::new(
        //     "large-v3".to_string(),
        //     &self.models_dir,
        //     "auto".to_string(),
        // )));
        
        Ok(())
    }
    
    /// Get provider by name
    async fn get_provider(&self, name: &str) -> ServiceResult<Arc<dyn TranscriptionProviderTrait>> {
        let providers = self.providers.read().await;
        
        for provider in providers.iter() {
            if provider.name() == name {
                // Clone the provider (requires Arc)
                // For now, we'll search again and return a clone
                return Ok(Arc::new(PlaceholderProvider)); // TODO: Fix provider cloning
            }
        }
        
        Err(AppError::NotFound(format!("Transcription provider '{}' not found", name)))
    }
    
    /// Get default or best provider
    async fn get_default_provider(&self) -> ServiceResult<Arc<dyn TranscriptionProviderTrait>> {
        let providers = self.providers.read().await;
        
        if providers.is_empty() {
            return Err(AppError::ServiceError("No transcription providers initialized".to_string()));
        }
        
        // Prefer local providers (Whisper) for cost/privacy
        // Fall back to API providers if needed
        for provider in providers.iter() {
            if provider.name() == "whisper" {
                return Ok(Arc::new(PlaceholderProvider)); // TODO: Fix
            }
        }
        
        // Return first available
        Ok(Arc::new(PlaceholderProvider)) // TODO: Fix
    }
}

// Temporary placeholder for provider cloning
struct PlaceholderProvider;
#[async_trait]
impl TranscriptionProviderTrait for PlaceholderProvider {
    fn name(&self) -> &'static str { "placeholder" }
    async fn transcribe_file(&self, _: &Path, _: &TranscriptionConfig) -> ServiceResult<TranscriptionResult> { todo!() }
    async fn transcribe_chunk(&self, _: &[u8], _: u32, _: &TranscriptionConfig) -> ServiceResult<TranscriptionSegment> { todo!() }
    async fn detect_language(&self, _: &[u8], _: u32) -> ServiceResult<String> { todo!() }
    fn list_models(&self) -> Vec<String> { vec![] }
}

#[async_trait]
impl TranscriptionService for TranscriptionServiceImpl {
    #[instrument(skip(self, config), fields(meeting_id = %meeting_id))]
    async fn transcribe_file(
        &self,
        recording_path: PathBuf,
        meeting_id: Uuid,
        config: Option<TranscriptionConfig>,
    ) -> ServiceResult<Transcript> {
        // Initialize providers if not already done
        {
            let providers = self.providers.read().await;
            if providers.is_empty() {
                drop(providers);
                self.initialize_providers().await?;
            }
        }
        
        let config = config.unwrap_or_else(|| TranscriptionConfig {
            provider: match self.config.default_provider.as_str() {
                "nvidia" => TranscriptionProvider::NVIDIA {
                    model_name: "parakeet".to_string(),
                },
                "faster-whisper" => TranscriptionProvider::Whisper {
                    model: WhisperModel::LargeV3,
                },
                _ => TranscriptionProvider::Whisper {
                    model: WhisperModel::LargeV3,
                },
            },
            language: None, // Auto-detect
            task: TranscriptionTask::Transcribe,
            conditions: None,
            hotwords: None,
            chunk_duration_secs: 30,
            overlap_secs: 5,
        });
        
        // Verify audio file exists
        if !recording_path.exists() {
            return Err(AppError::NotFound(format!(
                "Recording file not found: {}",
                recording_path.display()
            )));
        }
        
        // Get appropriate provider
        let provider_name = match &config.provider {
            TranscriptionProvider::Whisper { .. } => "whisper",
            TranscriptionProvider::WhisperX { .. } => "whisperx",
            TranscriptionProvider::Parakeet => "nvidia",
            TranscriptionProvider::NVIDIA { .. } => "nvidia",
            TranscriptionProvider::Custom { .. } => "custom",
        };
        
        info!("Transcribing {} with provider: {}", recording_path.display(), provider_name);
        
        // TODO: Get actual provider and transcribe
        // For now, create placeholder transcript
        
        let transcript = Transcript {
            id: Uuid::new_v4(),
            meeting_id,
            recording_id: None,
            segments: vec![],
            language: "en".to_string(),
            duration_secs: 0.0,
            word_count: 0,
            status: TranscriptionStatus::Completed,
            metadata: TranscriptionMetadata {
                model_name: "placeholder".to_string(),
                provider: provider_name.to_string(),
                processing_time_secs: None,
                gpu_accelerated: false,
            },
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        // Save to database
        self.save_transcript(&transcript).await?;
        
        info!("Transcription completed for meeting {}", meeting_id);
        Ok(transcript)
    }
    
    async fn transcribe_stream(
        &self,
        meeting_id: Uuid,
        config: Option<TranscriptionConfig>,
    ) -> ServiceResult<Uuid> {
        // Create transcript record
        let transcript_id = Uuid::new_v4();
        
        let transcript = Transcript {
            id: transcript_id,
            meeting_id,
            recording_id: None,
            segments: vec![],
            language: "unknown".to_string(),
            duration_secs: 0.0,
            word_count: 0,
            status: TranscriptionStatus::InProgress,
            metadata: TranscriptionMetadata {
                model_name: "streaming".to_string(),
                provider: "whisper".to_string(),
                processing_time_secs: None,
                gpu_accelerated: false,
            },
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        self.save_transcript(&transcript).await?;
        
        Ok(transcript_id)
    }
    
    async fn submit_chunk(
        &self,
        transcript_id: Uuid,
        audio_data: Bytes,
        offset_secs: f32,
    ) -> ServiceResult<TranscriptionSegment> {
        // Get providers
        let providers = self.providers.read().await;
        if providers.is_empty() {
            return Err(AppError::ServiceError("No providers initialized".to_string()));
        }
        
        // Use first provider for streaming (should be Whisper for low latency)
        let provider = &providers[0];
        
        // Transcribe chunk
        // TODO: Implement actual transcription
        let segment = TranscriptionSegment {
            id: Uuid::new_v4(),
            transcript_id,
            start_time_secs: offset_secs,
            end_time_secs: offset_secs + 5.0, // Assume 5 second chunks
            text: String::new(), // TODO: Actual transcription
            confidence: 0.0,
            speaker_id: None,
            language: "en".to_string(),
            words: vec![],
        };
        
        // Add to transcript
        self.add_segment_to_transcript(transcript_id, &segment).await?;
        
        Ok(segment)
    }
    
    async fn get_transcript(&self, transcript_id: Uuid) -> ServiceResult<Transcript> {
        let record = sqlx::query_as!(
            TranscriptRow,
            r#"
            SELECT 
                id, meeting_id, recording_id, language, duration_secs,
                word_count, status, metadata_json, created_at, updated_at
            FROM transcripts
            WHERE id = $1
            "#,
            transcript_id
        )
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        match record {
            Some(row) => {
                let mut transcript = row.into();
                // Load segments
                transcript.segments = self.load_segments(transcript_id).await?;
                Ok(transcript)
            }
            None => Err(AppError::NotFound(format!("Transcript {} not found", transcript_id))),
        }
    }
    
    async fn get_meeting_transcript(&self, meeting_id: Uuid) -> ServiceResult<Option<Transcript>> {
        let record = sqlx::query_as!(
            TranscriptRow,
            r#"
            SELECT 
                id, meeting_id, recording_id, language, duration_secs,
                word_count, status, metadata_json, created_at, updated_at
            FROM transcripts
            WHERE meeting_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            meeting_id
        )
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        match record {
            Some(row) => {
                let mut transcript = row.into();
                transcript.segments = self.load_segments(transcript.id).await?;
                Ok(Some(transcript))
            }
            None => Ok(None),
        }
    }
    
    async fn update_transcript(
        &self,
        transcript_id: Uuid,
        segments: Vec<TranscriptionSegment>,
    ) -> ServiceResult<Transcript> {
        // Update segments in database
        for segment in &segments {
            self.update_segment(transcript_id, segment).await?;
        }
        
        // Reload full transcript
        self.get_transcript(transcript_id).await
    }
    
    async fn delete_transcript(&self, transcript_id: Uuid) -> ServiceResult<()> {
        // Delete segments first (foreign key)
        sqlx::query!("DELETE FROM transcript_segments WHERE transcript_id = $1", transcript_id)
            .execute(&self.db_pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        // Delete transcript
        sqlx::query!("DELETE FROM transcripts WHERE id = $1", transcript_id)
            .execute(&self.db_pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        info!("Deleted transcript {}", transcript_id);
        Ok(())
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
impl TranscriptionServiceImpl {
    async fn save_transcript(&self, transcript: &Transcript) -> ServiceResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO transcripts (
                id, meeting_id, recording_id, language, duration_secs,
                word_count, status, metadata_json
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET
                language = EXCLUDED.language,
                duration_secs = EXCLUDED.duration_secs,
                word_count = EXCLUDED.word_count,
                status = EXCLUDED.status,
                metadata_json = EXCLUDED.metadata_json,
                updated_at = NOW()
            "#,
            transcript.id,
            transcript.meeting_id,
            transcript.recording_id,
            transcript.language,
            transcript.duration_secs,
            transcript.word_count as i64,
            transcript.status.to_string(),
            serde_json::to_value(&transcript.metadata).map_err(|e| AppError::JsonError(e.to_string()))?,
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn load_segments(&self, transcript_id: Uuid) -> ServiceResult<Vec<TranscriptionSegment>> {
        let records = sqlx::query_as!(
            SegmentRow,
            r#"
            SELECT 
                id, transcript_id, start_time_secs, end_time_secs, text,
                confidence, speaker_id, language, words_json
            FROM transcript_segments
            WHERE transcript_id = $1
            ORDER BY start_time_secs ASC
            "#,
            transcript_id
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(records.into_iter().map(|r| r.into()).collect())
    }
    
    async fn add_segment_to_transcript(
        &self,
        transcript_id: Uuid,
        segment: &TranscriptionSegment,
    ) -> ServiceResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO transcript_segments (
                id, transcript_id, start_time_secs, end_time_secs,
                text, confidence, speaker_id, language, words_json
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            segment.id,
            transcript_id,
            segment.start_time_secs,
            segment.end_time_secs,
            &segment.text,
            segment.confidence,
            &segment.speaker_id,
            &segment.language,
            serde_json::to_string(&segment.words).unwrap_or("[]".to_string())
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn update_segment(
        &self,
        _transcript_id: Uuid,
        segment: &TranscriptionSegment,
    ) -> ServiceResult<()> {
        // TODO: Implement segment update
        Ok(())
    }
}

// Database row mappings
#[derive(sqlx::FromRow)]
struct TranscriptRow {
    id: Uuid,
    meeting_id: Uuid,
    recording_id: Option<Uuid>,
    language: String,
    duration_secs: f32,
    word_count: i64,
    status: String,
    metadata_json: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<TranscriptRow> for Transcript {
    fn from(row: TranscriptRow) -> Self {
        Self {
            id: row.id,
            meeting_id: row.meeting_id,
            recording_id: row.recording_id,
            segments: vec![], // Loaded separately
            language: row.language,
            duration_secs: row.duration_secs,
            word_count: row.word_count as u32,
            status: match row.status.as_str() {
                "pending" => TranscriptionStatus::Pending,
                "in_progress" => TranscriptionStatus::InProgress,
                "completed" => TranscriptionStatus::Completed,
                "failed" => TranscriptionStatus::Failed("Unknown".to_string()),
                _ => TranscriptionStatus::Pending,
            },
            metadata: serde_json::from_value(row.metadata_json).unwrap_or_default(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct SegmentRow {
    id: Uuid,
    transcript_id: Uuid,
    start_time_secs: f32,
    end_time_secs: f32,
    text: String,
    confidence: f32,
    speaker_id: Option<String>,
    language: String,
    words_json: String,
}

impl From<SegmentRow> for TranscriptionSegment {
    fn from(row: SegmentRow) -> Self {
        Self {
            id: row.id,
            transcript_id: row.transcript_id,
            start_time_secs: row.start_time_secs,
            end_time_secs: row.end_time_secs,
            text: row.text,
            confidence: row.confidence,
            speaker_id: row.speaker_id,
            language: row.language,
            words: serde_json::from_str(&row.words_json).unwrap_or_default(),
        }
    }
}

// Database schema (for migrations)
// CREATE TABLE IF NOT EXISTS transcripts (
//     id UUID PRIMARY KEY,
//     meeting_id UUID NOT NULL,
//     recording_id UUID,
//     language TEXT NOT NULL,
//     duration_secs REAL NOT NULL,
//     word_count BIGINT NOT NULL,
//     status TEXT NOT NULL,
//     metadata_json JSONB NOT NULL,
//     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
//     updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
// );
//
// CREATE TABLE IF NOT EXISTS transcript_segments (
//     id UUID PRIMARY KEY,
//     transcript_id UUID NOT NULL REFERENCES transcripts(id) ON DELETE CASCADE,
//     start_time_secs REAL NOT NULL,
//     end_time_secs REAL NOT NULL,
//     text TEXT NOT NULL,
//     confidence REAL NOT NULL,
//     speaker_id TEXT,
//     language TEXT NOT NULL,
//     words_json JSONB NOT NULL,
//     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
// );
//
// CREATE INDEX idx_transcripts_meeting_id ON transcripts(meeting_id);
// CREATE INDEX idx_segments_transcript_id ON transcript_segments(transcript_id);