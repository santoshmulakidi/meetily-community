# Meetily Community+ - Phase 2: Service Interface Design

**Goal:** Create modular, testable, SOLID-compliant service abstractions for all core functionality.

**Design Principles:**
- **Single Responsibility:** Each service does one thing well
- **Dependency Inversion:** Depend on abstractions, not concretions
- **Interface Segregation:** Small, focused traits
- **Testability:** All services mockable via trait objects
- **Async-first:** Tokio-based async runtime

---

## Service Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                  Meetily Community+ Server                   │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              API Layer (Axum Router)                  │   │
│  │  ┌────────────────────────────────────────────────┐  │   │
│  │  │  Route Handlers (thin, delegate to services)   │  │   │
│  │  └────────────────────────────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────┘   │
│                         ↓ injects                           │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              Service Layer (Traits)                   │   │
│  │  ┌──────────┐ ┌───────────┐ ┌──────────┐ ┌────────┐ │   │
│  │  │Recording │ │Transcribe │ │Diarize   │ │Summary │ │   │
│  │  │Service   │ │Service    │ │Service   │ │Service │ │   │
│  │  └──────────┘ └───────────┘ └──────────┘ └────────┘ │   │
│  │  ┌──────────┐ ┌───────────┐ ┌──────────┐ ┌────────┐ │   │
│  │  │ Embed    │ │  Search   │ │   Chat   │ │  Auth  │ │   │
│  │  │ Service  │ │  Service  │ │ Service  │ │Service │ │   │
│  │  └──────────┘ └───────────┘ └──────────┘ └────────┘ │   │
│  └──────────────────────────────────────────────────────┘   │
│                         ↓ uses                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │           Repository Layer (Database Abstraction)     │   │
│  │  ┌────────────┐ ┌────────────┐ ┌─────────────────┐  │   │
│  │  │ MeetingRepo│ │TranscriptRepo│ │EmbeddingRepo  │  │   │
│  │  └────────────┘ └────────────┘ └─────────────────┘  │   │
│  └──────────────────────────────────────────────────────┘   │
│                         ↓ connects to                       │
│  ┌──────────────────────────────────────────────────────┐   │
│  │          Infrastructure (PostgreSQL + pgvector)        │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## 1. Core Service Traits

### 1.1 Recording Service

```rust
// services/recording/mod.rs
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
```

### 1.2 Transcription Service

```rust
// services/transcription/mod.rs
use async_trait::async_trait;
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
```

### 1.3 Diarization Service

```rust
// services/diarization/mod.rs
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
```

### 1.4 Summary Service

```rust
// services/summary/mod.rs
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ServiceResult;
use crate::services::transcription::Transcript;

/// Meeting summary structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingSummary {
    pub id: Uuid,
    pub transcript_id: Uuid,
    pub meeting_id: Uuid,
    pub executive_summary: String,
    pub technical_summary: Option<String>,
    pub action_items: Vec<ActionItem>,
    pub decisions: Vec<Decision>,
    pub risks: Vec<Risk>,
    pub follow_up_tasks: Vec<FollowUpTask>,
    pub topics: Vec<Topic>,
    pub sentiment: SentimentAnalysis,
    pub metadata: SummaryMetadata,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItem {
    pub id: String,
    pub description: String,
    pub assignee: Option<String>,
    pub due_date: Option<String>,
    pub priority: Priority,
    pub status: ActionItemStatus,
    pub timestamp_secs: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: String,
    pub description: String,
    pub context: String,
    pub timestamp_secs: Option<f32>,
    pub participants: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Risk {
    pub id: String,
    pub description: String,
    pub severity: Severity,
    pub mitigation: Option<String>,
    pub timestamp_secs: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowUpTask {
    pub id: String,
    pub description: String,
    pub owner: Option<String>,
    pub related_to: Option<String>,  // action_item_id or decision_id
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub name: String,
    pub keywords: Vec<String>,
    pub duration_secs: f32,
    pub timestamp_secs: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentAnalysis {
    pub overall: Sentiment,
    pub by_speaker: Vec<SpeakerSentiment>,
    pub timeline: Vec<TimelineSentiment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Sentiment {
    Positive,
    Neutral,
    Negative,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerSentiment {
    pub speaker_id: String,
    pub sentiment: Sentiment,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineSentiment {
    pub start_time_secs: f32,
    pub end_time_secs: f32,
    pub sentiment: Sentiment,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActionItemStatus {
    pub status: String,  // "Not Started", "In Progress", "Completed"
    pub updated_at: DateTime<Utc>,
}

/// Summary provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SummaryProvider {
    Ollama { model: String, base_url: String },
    OpenRouter { model: String },
    OpenAI { model: String },
    Anthropic { model: String },
    NVIDIA { model: String },
    Custom { endpoint: String },
}

/// Summary configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryConfig {
    pub provider: SummaryProvider,
    pub sections: Vec<SummarySection>,
    pub custom_prompt: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub chunking_strategy: ChunkingStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SummarySection {
    ExecutiveSummary,
    TechnicalSummary,
    ActionItems,
    Decisions,
    Risks,
    FollowUpTasks,
    Topics,
    Sentiment,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChunkingStrategy {
    Fixed { chunk_size_tokens: u32, overlap_tokens: u32 },
    Semantic { max_chunk_size: u32 },
    Section { sections: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryMetadata {
    pub model_used: String,
    pub provider_name: String,
    pub processing_time_secs: f32,
    pub tokens_used: u32,
    pub chunk_count: u32,
}

#[async_trait]
pub trait SummaryService: Send + Sync {
    /// Generate summary from transcript
    async fn generate_summary(
        &self,
        transcript_id: Uuid,
        config: Option<SummaryConfig>,
    ) -> ServiceResult<MeetingSummary>;

    /// Regenerate specific section
    async fn regenerate_section(
        &self,
        summary_id: Uuid,
        section: SummarySection,
    ) -> ServiceResult<()>;

    /// Get summary by ID
    async fn get_summary(&self, summary_id: Uuid) -> ServiceResult<MeetingSummary>;

    /// Get summary for meeting
    async fn get_meeting_summary(&self, meeting_id: Uuid) -> ServiceResult<Option<MeetingSummary>>;

    /// Update summary (user edits)
    async fn update_summary(
        &self,
        summary_id: Uuid,
        updates: SummaryUpdate,
    ) -> ServiceResult<MeetingSummary>;

    /// Delete summary
    async fn delete_summary(&self, summary_id: Uuid) -> ServiceResult<()>;

    /// Export summary to markdown/PDF
    async fn export_summary(
        &self,
        summary_id: Uuid,
        format: ExportFormat,
    ) -> ServiceResult<Vec<u8>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryUpdate {
    pub executive_summary: Option<String>,
    pub technical_summary: Option<String>,
    pub action_items: Option<Vec<ActionItem>>,
    pub decisions: Option<Vec<Decision>>,
    pub risks: Option<Vec<Risk>>,
    pub follow_up_tasks: Option<Vec<FollowUpTask>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Markdown,
    Pdf,
    Docx,
    Html,
}
```

---

## 2. Repository Layer (Database Abstraction)

### 2.1 Meeting Repository

```rust
// repositories/meeting/mod.rs
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::RepositoryResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meeting {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub status: MeetingStatus,
    pub metadata: MeetingMetadata,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MeetingStatus {
    Scheduled,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeetingMetadata {
    pub recording_count: u32,
    pub transcript_count: u32,
    pub summary_count: u32,
    pub total_duration_secs: f32,
    pub participant_count: u32,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MeetingFilter {
    pub user_id: Option<Uuid>,
    pub status: Option<MeetingStatus>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub search_query: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[async_trait]
pub trait MeetingRepository: Send + Sync {
    async fn create(&self, meeting: Meeting) -> RepositoryResult<Meeting>;
    async fn get_by_id(&self, id: Uuid) -> RepositoryResult<Option<Meeting>>;
    async fn update(&self, meeting: Meeting) -> RepositoryResult<Meeting>;
    async fn delete(&self, id: Uuid) -> RepositoryResult<()>;
    async fn list(&self, filter: MeetingFilter) -> RepositoryResult<Vec<Meeting>>;
    async fn get_current_meeting(&self, user_id: Uuid) -> RepositoryResult<Option<Meeting>>;
}
```

### 2.2 Embedding Repository (for pgvector)

```rust
// repositories/embedding/mod.rs
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::RepositoryResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub transcript_id: Option<Uuid>,
    pub chunk_text: String,
    pub embedding: Vec<f32>,  // pgvector vector
    pub chunk_index: u32,
    pub start_time_secs: f32,
    pub end_time_secs: f32,
    pub speaker_id: Option<String>,
    pub tokens: u32,
    pub model_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct VectorSearchQuery {
    pub query_embedding: Vec<f32>,
    pub meeting_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub limit: i64,
    pub similarity_threshold: f32,
    pub filters: SearchFilters,
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub speaker_id: Option<String>,
    pub min_confidence: Option<f32>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub embedding: Embedding,
    pub similarity_score: f32,
    pub meeting_title: String,
}

#[async_trait]
pub trait EmbeddingRepository: Send + Sync {
    async fn insert(&self, embedding: Embedding) -> RepositoryResult<()>;
    async fn insert_batch(&self, embeddings: Vec<Embedding>) -> RepositoryResult<()>;
    async fn search(&self, query: VectorSearchQuery) -> RepositoryResult<Vec<SearchMatch>>;
    async fn get_by_transcript_id(&self, transcript_id: Uuid) -> RepositoryResult<Vec<Embedding>>;
    async fn delete_by_transcript_id(&self, transcript_id: Uuid) -> RepositoryResult<()>;
}
```

---

## 3. Dependency Injection Container

```rust
// di/container.rs
use std::sync::Arc;

use crate::services::recording::RecordingService;
use crate::services::transcription::TranscriptionService;
use crate::services::diarization::DiarizationService;
use crate::services::summary::SummaryService;
use crate::repositories::meeting::MeetingRepository;
use crate::repositories::embedding::EmbeddingRepository;
use crate::config::AppConfig;

/// Application state shared across all handlers
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
```

---

## 4. Error Handling

```rust
// error/mod.rs
use thiserror::Error;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// Unified error type for all services
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Service error: {0}")]
    ServiceError(String),

    #[error("Repository error: {0}")]
    RepositoryError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("Authorization error: {0}")]
    AuthorizationError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("External service error: {provider} - {message}")]
    ExternalServiceError {
        provider: String,
        message: String,
    },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_response) = match &self {
            AppError::ValidationError(msg) => (
                StatusCode::BAD_REQUEST,
                ErrorResponse {
                    error: msg.clone(),
                    code: "VALIDATION_ERROR".to_string(),
                    details: None,
                },
            ),
            AppError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                ErrorResponse {
                    error: msg.clone(),
                    code: "NOT_FOUND".to_string(),
                    details: None,
                },
            ),
            AppError::Conflict(msg) => (
                StatusCode::CONFLICT,
                ErrorResponse {
                    error: msg.clone(),
                    code: "CONFLICT".to_string(),
                    details: None,
                },
            ),
            AppError::AuthError(msg) => (
                StatusCode::UNAUTHORIZED,
                ErrorResponse {
                    error: msg.clone(),
                    code: "UNAUTHORIZED".to_string(),
                    details: None,
                },
            ),
            AppError::AuthorizationError(msg) => (
                StatusCode::FORBIDDEN,
                ErrorResponse {
                    error: msg.clone(),
                    code: "FORBIDDEN".to_string(),
                    details: None,
                },
            ),
            AppError::ExternalServiceError { provider, message } => (
                StatusCode::BAD_GATEWAY,
                ErrorResponse {
                    error: format!("{}: {}", provider, message),
                    code: "EXTERNAL_SERVICE_ERROR".to_string(),
                    details: None,
                },
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorResponse {
                    error: self.to_string(),
                    code: "INTERNAL_ERROR".to_string(),
                    details: None,
                },
            ),
        };

        (status, Json(error_response)).into_response()
    }
}

/// Type aliases for cleaner code
pub type ServiceResult<T> = Result<T, AppError>;
pub type RepositoryResult<T> = Result<T, AppError>;
```

---

## 5. Configuration Management

```rust
// config/mod.rs
use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub storage: StorageConfig,
    pub transcription: TranscriptionConfig,
    pub summary: SummaryProviderConfig,
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub log_level: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub recordings_path: String,
    pub max_file_size_mb: u64,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TranscriptionConfig {
    pub default_provider: String,
    pub whisper_model: String,
    pub nvidia_api_key: Option<String>,
    pub nvidia_base_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SummaryProviderConfig {
    pub default_provider: String,
    pub ollama_base_url: String,
    pub ollama_model: String,
    pub openrouter_api_key: Option<String>,
    pub openrouter_model: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub token_expiry_hours: u64,
    pub refresh_token_expiry_days: u64,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, config::ConfigError> {
        // Use config crate for env-based loading with defaults
        let config = config::Config::builder()
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8080)?
            .set_default("server.log_level", "info")?
            .set_default("database.max_connections", 10)?
            .set_default("database.min_connections", 2)?
            .set_default("storage.max_file_size_mb", 1024)?
            .set_default("storage.retention_days", 30)?
            .set_default("transcription.default_provider", "whisper")?
            .set_default("transcription.whisper_model", "large-v3")?
            .set_default("transcription.nvidia_base_url", "https://integrate.api.nvidia.com/v1")?
            .set_default("summary.default_provider", "ollama")?
            .set_default("summary.ollama_model", "llama3.1:8b")?
            .set_default("summary.ollama_base_url", "http://localhost:11434")?
            .add_source(config::Environment::with_prefix("MEETELY").separator("__"))
            .build()?;

        config.try_deserialize()
    }
}
```

---

## Next Steps

This design provides:
✅ **SOLID interfaces** - Each service has single responsibility  
✅ **Dependency inversion** - Services depend on repository traits  
✅ **Testability** - All traits mockable for unit tests  
✅ **Provider extensibility** - Easy to add new transcription/summary providers  
✅ **Type safety** - Strong typing with serde for serialization  
✅ **Async-native** - All traits use `async_trait`

**Ready to implement!** I'll proceed with:
1. Creating the project structure (Cargo workspace)
2. Implementing each service trait with concrete implementations
3. Building mock implementations for testing
4. Setting up CI/CD with unit tests

**Awaiting approval** to continue with implementation.