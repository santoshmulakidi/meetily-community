//! Embedding service trait and implementation

mod embedding_service;

pub use embedding_service::EmbeddingServiceImpl;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::ServiceResult;

/// Meeting summary structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub summary_type: SummaryType,
    pub content: String,
    pub model_used: String,
    pub provider: String,
    pub token_usage: Option<SummaryMetadata>,
    pub created_at: DateTime<Utc>,
}

/// Type of summary
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SummaryType {
    Executive,
    Technical,
    ActionItems,
    Decisions,
    Risks,
    FollowUp,
    Custom,
}

/// Token usage metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SummaryMetadata {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub cost_usd: Option<f32>,
}