//! Embedding repository trait and PostgreSQL implementation (with pgvector)

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

/// Embedding repository trait
#[async_trait]
pub trait EmbeddingRepository: Send + Sync {
    async fn insert(&self, embedding: Embedding) -> RepositoryResult<()>;
    async fn insert_batch(&self, embeddings: Vec<Embedding>) -> RepositoryResult<()>;
    async fn search(&self, query: VectorSearchQuery) -> RepositoryResult<Vec<SearchMatch>>;
    async fn get_by_transcript_id(&self, transcript_id: Uuid) -> RepositoryResult<Vec<Embedding>>;
    async fn delete_by_transcript_id(&self, transcript_id: Uuid) -> RepositoryResult<()>;
}

/// PostgreSQL implementation with pgvector
pub struct PostgresEmbeddingRepository {
    pool: sqlx::PgPool,
}

impl PostgresEmbeddingRepository {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EmbeddingRepository for PostgresEmbeddingRepository {
    async fn insert(&self, embedding: Embedding) -> RepositoryResult<()> {
        todo!("Implement pgvector insertion")
    }

    async fn insert_batch(&self, embeddings: Vec<Embedding>) -> RepositoryResult<()> {
        todo!()
    }

    async fn search(&self, query: VectorSearchQuery) -> RepositoryResult<Vec<SearchMatch>> {
        todo!("Implement pgvector similarity search")
    }

    async fn get_by_transcript_id(&self, transcript_id: Uuid) -> RepositoryResult<Vec<Embedding>> {
        todo!()
    }

    async fn delete_by_transcript_id(&self, transcript_id: Uuid) -> RepositoryResult<()> {
        todo!()
    }
}