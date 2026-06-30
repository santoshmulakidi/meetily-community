//! Analytics service trait and implementation

mod analytics_service;

pub use analytics_service::AnalyticsServiceImpl;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::ServiceResult;

/// Time range for filtering analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeRange {
    pub fn to_date_range(&self) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
        (Some(self.start), Some(self.end))
    }
    
    pub fn last_7_days() -> Self {
        Self {
            start: Utc::now() - chrono::Duration::days(7),
            end: Utc::now(),
        }
    }
    
    pub fn last_30_days() -> Self {
        Self {
            start: Utc::now() - chrono::Duration::days(30),
            end: Utc::now(),
        }
    }
    
    pub fn last_90_days() -> Self {
        Self {
            start: Utc::now() - chrono::Duration::days(90),
            end: Utc::now(),
        }
    }
}

/// Meeting statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingStats {
    pub total_meetings: u32,
    pub total_hours: f32,
    pub avg_duration_mins: u32,
    pub with_transcripts: u32,
    pub with_summaries: u32,
    pub daily_trend: HashMap<String, i64>,
}

/// Speaker statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerStats {
    pub speaker_id: String,
    pub speaker_name: Option<String>,
    pub segment_count: u32,
    pub total_talk_time_secs: f32,
    pub avg_confidence: f32,
    pub meetings_participated: u32,
    pub talk_time_percentage: f32,
}

/// Topic analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicAnalytics {
    pub top_topics: Vec<(String, i64)>,
    pub summary_type_distribution: HashMap<String, i64>,
    pub topic_evolution: HashMap<String, Vec<(String, i64)>>,
}

/// Sentiment summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentSummary {
    pub overall_sentiment: String,
    pub sentiment_score: f32,
    pub positive_meetings: u32,
    pub neutral_meetings: u32,
    pub negative_meetings: u32,
    pub sentiment_trend: Vec<HashMap<String, serde_json::Value>>,
}

/// Action item statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItemStats {
    pub total_items: u32,
    pub completed_items: u32,
    pub pending_items: u32,
    pub overdue_items: u32,
    pub completion_rate: f32,
    pub top_owners: Vec<SpeakerStats>,
}

/// Dashboard response (aggregated analytics)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardResponse {
    pub meeting_stats: MeetingStats,
    pub speaker_stats: Vec<SpeakerStats>,
    pub topic_analytics: TopicAnalytics,
    pub sentiment_summary: SentimentSummary,
    pub action_items: ActionItemStats,
    pub generated_at: DateTime<Utc>,
}

/// Analytics service trait
#[async_trait]
pub trait AnalyticsService: Send + Sync {
    async fn get_meeting_stats(&self, time_range: Option<TimeRange>) -> ServiceResult<MeetingStats>;
    async fn get_speaker_stats(&self, meeting_id: Option<Uuid>) -> ServiceResult<Vec<SpeakerStats>>;
    async fn get_topic_analytics(&self) -> ServiceResult<TopicAnalytics>;
    async fn get_sentiment_summary(&self, meeting_ids: Option<Vec<Uuid>>) -> ServiceResult<SentimentSummary>;
    async fn get_action_item_stats(&self) -> ServiceResult<ActionItemStats>;
    async fn get_dashboard(&self, time_range: Option<TimeRange>) -> ServiceResult<DashboardResponse>;
    async fn get_meeting_trends(&self, period_days: u32) -> ServiceResult<Vec<HashMap<String, serde_json::Value>>>;
}