//! Analytics service implementation
//!
//! Features:
//! - Meeting statistics (count, duration, participants, trends)
//! - Speaker analytics (talk time, participation rates)
//! - Topic analytics (clustering, evolution over time)
//! - Sentiment analysis integration
//! - Action item tracking (completion rates, overdue)
//! - Dashboard API endpoints

use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use uuid::Uuid;
use tracing::{info, warn, error, debug, instrument};
use std::collections::HashMap;

use crate::error::{AppError, ServiceResult};
use super::{
    AnalyticsService, MeetingStats, SpeakerStats, TopicAnalytics, 
    SentimentSummary, ActionItemStats, DashboardResponse, TimeRange,
};

/// Main analytics service implementation
pub struct AnalyticsServiceImpl {
    db_pool: Pool<Postgres>,
}

impl AnalyticsServiceImpl {
    /// Create a new analytics service
    pub fn new(db_pool: Pool<Postgres>) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl AnalyticsService for AnalyticsServiceImpl {
    #[instrument(skip(self))]
    async fn get_meeting_stats(&self, time_range: Option<TimeRange>) -> ServiceResult<MeetingStats> {
        // Build time range filter
        let (start_date, end_date) = match time_range {
            Some(range) => range.to_date_range(),
            None => (None, None),
        };

        // Total meetings count
        let total_query = if start_date.is_some() || end_date.is_some() {
            r#"
            SELECT COUNT(*) as count
            FROM meetings
            WHERE ($1::TIMESTAMPTZ IS NULL OR created_at >= $1)
              AND ($2::TIMESTAMPTZ IS NULL OR created_at <= $2)
            "#
        } else {
            r#"SELECT COUNT(*) as count FROM meetings"#
        };

        let total = sqlx::query_scalar!(
            total_query,
            start_date,
            end_date
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Total recording hours
        let hours_query = if start_date.is_some() || end_date.is_some() {
            r#"
            SELECT COALESCE(SUM(duration_secs), 0) as total_secs
            FROM recordings
            WHERE meeting_id IN (
                SELECT id FROM meetings
                WHERE ($1::TIMESTAMPTZ IS NULL OR created_at >= $1)
                  AND ($2::TIMESTAMPTZ IS NULL OR created_at <= $2)
            )
            "#
        } else {
            r#"
            SELECT COALESCE(SUM(duration_secs), 0) as total_secs
            FROM recordings
            WHERE meeting_id IN (SELECT id FROM meetings)
            "#
        };

        let total_secs: i64 = sqlx::query_scalar!(
            hours_query,
            start_date,
            end_date
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Average meeting duration
        let avg_duration_query = if start_date.is_some() || end_date.is_some() {
            r#"
            SELECT AVG(r.duration_secs) as avg_secs
            FROM recordings r
            JOIN meetings m ON r.meeting_id = m.id
            WHERE ($1::TIMESTAMPTZ IS NULL OR m.created_at >= $1)
              AND ($2::TIMESTAMPTZ IS NULL OR m.created_at <= $2)
            "#
        } else {
            r#"
            SELECT AVG(duration_secs) as avg_secs FROM recordings
            "#
        };

        let avg_secs: Option<f64> = sqlx::query_scalar!(
            avg_duration_query,
            start_date,
            end_date
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Meetings with transcripts
        let with_transcripts = sqlx::query_scalar!(
            r#"
            SELECT COUNT(DISTINCT m.id)
            FROM meetings m
            JOIN transcripts t ON m.id = t.meeting_id
            WHERE ($1::TIMESTAMPTZ IS NULL OR m.created_at >= $1)
              AND ($2::TIMESTAMPTZ IS NULL OR m.created_at <= $2)
            "#,
            start_date,
            end_date
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Meetings with summaries
        let with_summaries = sqlx::query_scalar!(
            r#"
            SELECT COUNT(DISTINCT m.id)
            FROM meetings m
            JOIN summaries s ON m.id = s.meeting_id
            WHERE ($1::TIMESTAMPTZ IS NULL OR m.created_at >= $1)
              AND ($2::TIMESTAMPTZ IS NULL OR m.created_at <= $2)
            "#,
            start_date,
            end_date
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Meetings per day (last 30 days)
        let daily_trend = sqlx::query!(
            r#"
            SELECT 
                DATE(created_at) as date,
                COUNT(*) as count
            FROM meetings
            WHERE created_at >= NOW() - INTERVAL '30 days'
              AND ($1::TIMESTAMPTZ IS NULL OR created_at >= $1)
              AND ($2::TIMESTAMPTZ IS NULL OR created_at <= $2)
            GROUP BY DATE(created_at)
            ORDER BY date ASC
            "#,
            start_date,
            end_date
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let daily_counts: HashMap<String, i64> = daily_trend
            .into_iter()
            .map(|r| (r.date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default(), r.count))
            .collect();

        Ok(MeetingStats {
            total_meetings: total as u32,
            total_hours: (total_secs as f32 / 3600.0).round(),
            avg_duration_mins: avg_secs.map(|s| (s / 60.0).round() as u32).unwrap_or(0),
            with_transcripts: with_transcripts as u32,
            with_summaries: with_summaries as u32,
            daily_trend: daily_counts,
        })
    }

    #[instrument(skip(self))]
    async fn get_speaker_stats(&self, meeting_id: Option<Uuid>) -> ServiceResult<Vec<SpeakerStats>> {
        // If meeting_id provided, get stats for that meeting only
        // Otherwise, aggregate across all meetings

        let query = if meeting_id.is_some() {
            r#"
            SELECT 
                COALESCE(ds.speaker_id, 'Unknown') as speaker_id,
                COALESCE(sa.alias, ds.speaker_id) as speaker_name,
                COUNT(DISTINCT ds.segment_id) as segment_count,
                SUM(COALESCE(ds.duration_secs, 0)) as total_talk_time_secs,
                AVG(COALESCE(ds.confidence, 0)) as avg_confidence,
                COUNT(DISTINCT d.meeting_id) as meetings_participated
            FROM diarization_segments ds
            JOIN diarizations d ON ds.diarization_id = d.id
            LEFT JOIN speaker_aliases sa ON ds.speaker_id = sa.speaker_id AND sa.meeting_id = d.meeting_id
            WHERE ($1::UUID IS NULL OR d.meeting_id = $1)
            GROUP BY ds.speaker_id, sa.alias, sa.meeting_id
            ORDER BY total_talk_time_secs DESC
            LIMIT 50
            "#
        } else {
            r#"
            SELECT 
                COALESCE(ds.speaker_id, 'Unknown') as speaker_id,
                COUNT(DISTINCT ds.segment_id) as segment_count,
                SUM(COALESCE(ds.duration_secs, 0)) as total_talk_time_secs,
                AVG(COALESCE(ds.confidence, 0)) as avg_confidence,
                COUNT(DISTINCT d.meeting_id) as meetings_participated
            FROM diarization_segments ds
            JOIN diarizations d ON ds.diarization_id = d.id
            WHERE $1::UUID IS NULL OR d.meeting_id = $1
            GROUP BY ds.speaker_id
            ORDER BY meetings_participated DESC, total_talk_time_secs DESC
            LIMIT 50
            "#
        };

        let rows = sqlx::query!(
            query,
            meeting_id
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let speaker_stats: Vec<SpeakerStats> = rows.into_iter().map(|r| {
            SpeakerStats {
                speaker_id: r.speaker_id,
                speaker_name: r.speaker_name,
                segment_count: r.segment_count as u32,
                total_talk_time_secs: r.total_talk_time_secs.unwrap_or(0.0),
                avg_confidence: r.avg_confidence.unwrap_or(0.0),
                meetings_participated: r.meetings_participated as u32,
                talk_time_percentage: 0.0, // Calculate later
            }
        }).collect();

        // Calculate talk time percentages
        let total_time: f32 = speaker_stats.iter().map(|s| s.total_talk_time_secs).sum();
        let mut speaker_stats: Vec<SpeakerStats> = speaker_stats.into_iter().map(|mut s| {
            s.talk_time_percentage = if total_time > 0.0 {
                (s.total_talk_time_secs / total_time * 100.0).round()
            } else {
                0.0
            };
            s
        }).collect();

        Ok(speaker_stats)
    }

    #[instrument(skip(self))]
    async fn get_topic_analytics(&self) -> ServiceResult<TopicAnalytics> {
        // Extract topics from summaries using keyword frequency
        // In production, use LLM-based topic clustering

        // Get most common words from transcripts (excluding stopwords)
        let common_topics = sqlx::query!(
            r#"
            WITH words AS (
                SELECT 
                    LOWER(word) as word,
                    COUNT(*) as frequency
                FROM (
                    SELECT 
                        REGEXP_SPLIT_TO_TABLE(LOWER(ts_text), E'\\s+') as word
                    FROM (
                        SELECT STRING_AGG(text, ' ') as ts_text
                        FROM transcript_segments
                        WHERE LENGTH(text) > 3
                    ) subq
                ) word_subq
                WHERE LENGTH(word) > 3
                  AND word NOT IN (
                      'the', 'and', 'that', 'have', 'for', 'this', 'with', 
                      'you', 'are', 'was', 'will', 'from', 'have', 'been',
                      'but', 'not', 'they', 'all', 'their', 'which', 'what',
                      'more', 'when', 'into', 'such', 'can', 'would', 'make',
                      'than', 'just', 'over', 'some', 'very', 'after', 'most',
                      'also', 'made', 'like', 'only', 'could', 'about', 'into',
                      'there', 'time', 'people', 'year', 'way', 'day', 'man',
                      'thing', 'woman', 'life', 'child', 'world', 'school',
                      'state', 'family', 'student', 'group', 'country', 'problem',
                      'hand', 'part', 'place', 'case', 'week', 'company',
                      'where', 'system', 'program', 'question', 'work', 'night',
                      'point', 'home', 'water', 'room', 'mother', 'area', 'money',
                      'story', 'fact', 'month', 'lot', 'right', 'study', 'book',
                      'eye', 'job', 'word', 'business', 'issue', 'look', 'process',
                      'name', 'number', 'question', 'need', 'stay', 'find', 'long',
                      'down', 'own', 'side', 'once', 'put', 'both', 'even', 'new',
                      'may', 'take', 'come', 'get', 'well', 'our', 'go', 'see',
                      'use', 'how', 'may', 'say', 'don', 'll', 're', 've', 'it',
                      'is', 'on', 'as', 'at', 'or', 'an', 'be', 'we', 'his', 'has',
                      'her', 'him', 'she', 'he', 'so', 'if', 'do', 'me', 'my', 'us'
                  )
                GROUP BY word
                ORDER BY frequency DESC
                LIMIT 20
            )
            SELECT word, frequency
            FROM words
            "#
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let topics: Vec<(String, i64)> = common_topics
            .into_iter()
            .map(|r| (r.word, r.frequency))
            .collect();

        // Get summary types distribution
        let summary_types = sqlx::query!(
            r#"
            SELECT summary_type, COUNT(*) as count
            FROM summaries
            GROUP BY summary_type
            ORDER BY count DESC
            "#
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let type_distribution: HashMap<String, i64> = summary_types
            .into_iter()
            .map(|r| (r.summary_type.unwrap_or_default(), r.count))
            .collect();

        Ok(TopicAnalytics {
            top_topics: topics,
            summary_type_distribution: type_distribution,
            topic_evolution: HashMap::new(), // TODO: Implement time-series topic tracking
        })
    }

    #[instrument(skip(self))]
    async fn get_sentiment_summary(&self, meeting_ids: Option<Vec<Uuid>>) -> ServiceResult<SentimentSummary> {
        // Placeholder: In production, integrate with sentiment analysis API
        // For now, return mock data based on meeting count

        let meeting_count = if let Some(ids) = meeting_ids {
            ids.len()
        } else {
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM meetings")
                .fetch_one(&self.db_pool)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
            count as usize
        };

        Ok(SentimentSummary {
            overall_sentiment: "neutral".to_string(),
            sentiment_score: 0.5,
            positive_meetings: (meeting_count as f32 * 0.4) as u32,
            neutral_meetings: (meeting_count as f32 * 0.5) as u32,
            negative_meetings: (meeting_count as f32 * 0.1) as u32,
            sentiment_trend: vec![], // TODO: Implement time-series
        })
    }

    #[instrument(skip(self))]
    async fn get_action_item_stats(&self) -> ServiceResult<ActionItemStats> {
        // Extract action items from summaries
        // For now, use placeholder calculations

        let total_summaries = sqlx::query_scalar!("SELECT COUNT(*) FROM summaries")
            .fetch_one(&self.db_pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let action_item_summaries = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) 
            FROM summaries 
            WHERE summary_type = 'action_items'
            "#
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // In production, parse action items and track completion
        // For now, return estimates
        let estimated_total_items = action_item_summaries * 5; // Average 5 items per summary
        let estimated_completed = (estimated_total_items as f32 * 0.6) as i64;
        let estimated_overdue = (estimated_total_items as f32 * 0.15) as i64;

        Ok(ActionItemStats {
            total_items: estimated_total_items as u32,
            completed_items: estimated_completed as u32,
            pending_items: (estimated_total_items - estimated_completed) as u32,
            overdue_items: estimated_overdue as u32,
            completion_rate: 0.6,
            top_owners: vec![], // TODO: Extract from action items
        })
    }

    #[instrument(skip(self))]
    async fn get_dashboard(&self, time_range: Option<TimeRange>) -> ServiceResult<DashboardResponse> {
        let meeting_stats = self.get_meeting_stats(time_range.clone()).await?;
        let speaker_stats = self.get_speaker_stats(None).await?;
        let topic_analytics = self.get_topic_analytics().await?;
        let sentiment = self.get_sentiment_summary(None).await?;
        let action_items = self.get_action_item_stats().await?;

        Ok(DashboardResponse {
            meeting_stats,
            speaker_stats: speaker_stats.into_iter().take(10).collect(),
            topic_analytics,
            sentiment_summary: sentiment,
            action_items,
            generated_at: Utc::now(),
        })
    }

    #[instrument(skip(self))]
    async fn get_meeting_trends(&self, period_days: u32) -> ServiceResult<Vec<HashMap<String, serde_json::Value>>> {
        let trends = sqlx::query!(
            r#"
            SELECT 
                DATE(created_at) as date,
                COUNT(*) as meeting_count,
                COALESCE(SUM(r.duration_secs), 0) as total_duration_secs,
                COUNT(DISTINCT t.id) as transcripts_count,
                COUNT(DISTINCT s.id) as summaries_count
            FROM meetings m
            LEFT JOIN recordings r ON m.id = r.meeting_id
            LEFT JOIN transcripts t ON m.id = t.meeting_id
            LEFT JOIN summaries s ON m.id = s.meeting_id
            WHERE m.created_at >= NOW() - ($1 || ' days')::INTERVAL
            GROUP BY DATE(m.created_at)
            ORDER BY date ASC
            "#,
            period_days.to_string()
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(trends.into_iter().map(|r| {
            let mut map = HashMap::new();
            map.insert("date".to_string(), serde_json::Value::String(
                r.date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default()
            ));
            map.insert("meeting_count".to_string(), serde_json::Value::Number(r.meeting_count.into()));
            map.insert("total_duration_mins".to_string(), serde_json::Value::Number(
                ((r.total_duration_secs.unwrap_or(0) as f32) / 60.0).round().into()
            ));
            map.insert("transcripts_count".to_string(), serde_json::Value::Number(r.transcripts_count.into()));
            map.insert("summaries_count".to_string(), serde_json::Value::Number(r.summaries_count.into()));
            map
        }).collect())
    }
}