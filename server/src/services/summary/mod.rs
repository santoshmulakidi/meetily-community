//! Summary service trait and implementation

mod summary_service;

pub use summary_service::SummaryServiceImpl;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::ServiceResult;

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
pub struct action_item {
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

/// Summary service trait
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

/// Placeholder implementation - to be filled in Phase 6
pub struct SummaryServiceImpl {
    // Configuration and resources
}

impl SummaryServiceImpl {
    pub fn new(_config: SummaryProviderConfig, _db_pool: sqlx::PgPool) -> Self {
        Self {
            // Initialize resources
        }
    }
}

#[async_trait]
impl SummaryService for SummaryServiceImpl {
    async fn generate_summary(
        &self,
        _transcript_id: Uuid,
        _config: Option<SummaryConfig>,
    ) -> ServiceResult<MeetingSummary> {
        todo!("Implement in Phase 6")
    }

    async fn regenerate_section(
        &self,
        _summary_id: Uuid,
        _section: SummarySection,
    ) -> ServiceResult<()> {
        todo!()
    }

    async fn get_summary(&self, _summary_id: Uuid) -> ServiceResult<MeetingSummary> {
        todo!()
    }

    async fn get_meeting_summary(&self, _meeting_id: Uuid) -> ServiceResult<Option<MeetingSummary>> {
        todo!()
    }

    async fn update_summary(
        &self,
        _summary_id: Uuid,
        _updates: SummaryUpdate,
    ) -> ServiceResult<MeetingSummary> {
        todo!()
    }

    async fn delete_summary(&self, _summary_id: Uuid) -> ServiceResult<()> {
        todo!()
    }

    async fn export_summary(
        &self,
        _summary_id: Uuid,
        _format: ExportFormat,
    ) -> ServiceResult<Vec<u8>> {
        todo!()
    }
}