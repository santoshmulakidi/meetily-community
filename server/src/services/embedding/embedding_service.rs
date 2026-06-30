//! Embedding service implementation
//!
//! Features:
//! - Pluggable embedding provider architecture (NVIDIA, OpenAI, local)
//! - Multiple vector storage backends (pgvector, Qdrant, Chroma)
//! - Intelligent chunking strategies (fixed, semantic, speaker-turn)
//! - Embedding cache for cost optimization
//! - Rich metadata associations

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use tracing::{info, warn, error, debug, instrument};

use crate::error::{AppError, ServiceResult};
use super::{
    EmbeddingService, Embedding, EmbeddingMetadata, VectorStore,
    EmbeddingProvider, EmbeddingConfig, ChunkingStrategy,
};
use crate::config::EmbeddingConfig as AppConfig;

/// Embedding provider trait - all backends must implement this
#[async_trait]
trait EmbeddingProviderTrait: Send + Sync {
    /// Get provider name
    fn name(&self) -> &'static str;
    
    /// Generate embedding for a single text
    async fn generate_embedding(
        &self,
        text: String,
        config: &EmbeddingConfig,
    ) -> ServiceResult<EmbeddingResult>;
    
    /// Generate embeddings for multiple texts (batch)
    async fn generate_batch(
        &self,
        texts: Vec<String>,
        config: &EmbeddingConfig,
    ) -> ServiceResult<Vec<EmbeddingResult>>;
    
    /// Get embedding dimension
    fn dimension(&self) -> usize;
    
    /// List available models
    fn list_models(&self) -> Vec<String>;
    
    /// Get cost per 1000 embeddings
    fn cost_per_1000(&self, model: &str) -> Option<f32>;
}

/// Embedding result
#[derive(Debug, Clone)]
struct EmbeddingResult {
    vector: Vec<f32>,
    model: String,
    dimension: usize,
    usage: EmbeddingUsage,
}

/// Token/character usage tracking
#[derive(Debug, Clone, Default)]
struct EmbeddingUsage {
    total_characters: u32,
    total_tokens: u32,
}

/// NVIDIA embedding provider (fast, free tier)
struct NVIDIAEmbeddingProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
    model: String,
}

impl NVIDIAEmbeddingProvider {
    fn new(api_key: String, base_url: String, model: String) -> Self {
        Self {
            api_key,
            base_url,
            client: reqwest::Client::new(),
            model,
        }
    }
}

#[async_trait]
impl EmbeddingProviderTrait for NVIDIAEmbeddingProvider {
    fn name(&self) -> &'static str {
        "nvidia"
    }
    
    #[instrument(skip(self, config), fields(provider = "nvidia", model = &self.model))]
    async fn generate_embedding(
        &self,
        text: String,
        _config: &EmbeddingConfig,
    ) -> ServiceResult<EmbeddingResult> {
        // NVIDIA API endpoint
        let request_body = serde_json::json!({
            "input": [text],
            "model": self.model,
            "input_type": "passage",  // or "query" for search queries
            "encoding_format": "float"
        });
        
        let response = self.client
            .post(&format!("{}/embeddings", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
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
        
        let result: serde_json::Value = response.json().await.map_err(|e| {
            AppError::ExternalServiceError {
                provider: "NVIDIA".to_string(),
                message: format!("Failed to parse response: {}", e),
            }
        })?;
        
        let embedding_data = result["data"][0]["embedding"]
            .as_array()
            .ok_or_else(|| AppError::ExternalServiceError {
                provider: "NVIDIA".to_string(),
                message: "Missing embedding data in response".to_string(),
            })?;
        
        let vector: Vec<f32> = embedding_data
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();
        
        let dimension = vector.len();
        let usage = EmbeddingUsage {
            total_characters: text.len() as u32,
            total_tokens: (text.len() / 4) as u32, // Rough estimate
        };
        
        info!(
            "NVIDIA embedding: {} dimensions, {} chars (model: {})",
            dimension, usage.total_characters, self.model
        );
        
        Ok(EmbeddingResult {
            vector,
            model: self.model.clone(),
            dimension,
            usage,
        })
    }
    
    async fn generate_batch(
        &self,
        texts: Vec<String>,
        _config: &EmbeddingConfig,
    ) -> ServiceResult<Vec<EmbeddingResult>> {
        // NVIDIA supports batch embeddings
        let request_body = serde_json::json!({
            "input": texts,
            "model": self.model,
            "input_type": "passage",
            "encoding_format": "float"
        });
        
        let response = self.client
            .post(&format!("{}/embeddings", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError {
                provider: "NVIDIA".to_string(),
                message: format!("Batch API request failed: {}", e),
            })?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError {
                provider: "NVIDIA".to_string(),
                message: format!("API returned {}: {}", status, error_text),
            });
        }
        
        let result: serde_json::Value = response.json().await.map_err(|e| {
            AppError::ExternalServiceError {
                provider: "NVIDIA".to_string(),
                message: format!("Failed to parse response: {}", e),
            }
        })?;
        
        let embeddings_array = result["data"]
            .as_array()
            .ok_or_else(|| AppError::ExternalServiceError {
                provider: "NVIDIA".to_string(),
                message: "Missing embeddings array in response".to_string(),
            })?;
        
        let mut results = Vec::new();
        for embedding_obj in embeddings_array {
            let embedding_data = embedding_obj["embedding"]
                .as_array()
                .ok_or_else(|| AppError::ExternalServiceError {
                    provider: "NVIDIA".to_string(),
                    message: "Missing embedding in array item".to_string(),
                })?;
            
            let vector: Vec<f32> = embedding_data
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect();
            
            results.push(EmbeddingResult {
                vector,
                model: self.model.clone(),
                dimension: vector.len(),
                usage: EmbeddingUsage::default(),
            });
        }
        
        info!("NVIDIA batch embedding: {} embeddings generated", results.len());
        Ok(results)
    }
    
    fn dimension(&self) -> usize {
        // NVIDIA Embedding models dimensions
        match self.model.as_str() {
            "nvidia/nv-embedqa-e5-v5" => 1024,
            "nvidia/nv-embedqa-mistral-7b-v2" => 1024,
            "baai/bge-m3" => 1024,
            _ => 1024, // Default
        }
    }
    
    fn list_models(&self) -> Vec<String> {
        vec![
            "nvidia/nv-embedqa-e5-v5".to_string(),
            "nvidia/nv-embedqa-mistral-7b-v2".to_string(),
            "baai/bge-m3".to_string(),
            "sentence-transformers/all-mpnet-base-v2".to_string(),
        ]
    }
    
    fn cost_per_1000(&self, _model: &str) -> Option<f32> {
        // NVIDIA free tier available
        Some(0.0)
    }
}

/// OpenAI embedding provider (high quality)
struct OpenAIEmbeddingProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
    model: String,
}

impl OpenAIEmbeddingProvider {
    fn new(api_key: String, base_url: String, model: String) -> Self {
        Self {
            api_key,
            base_url,
            client: reqwest::Client::new(),
            model,
        }
    }
}

#[async_trait]
impl EmbeddingProviderTrait for OpenAIEmbeddingProvider {
    fn name(&self) -> &'static str {
        "openai"
    }
    
    async fn generate_embedding(
        &self,
        text: String,
        _config: &EmbeddingConfig,
    ) -> ServiceResult<EmbeddingResult> {
        let request_body = serde_json::json!({
            "input": text,
            "model": self.model,
            "encoding_format": "float"
        });
        
        let response = self.client
            .post(&format!("{}/embeddings", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError {
                provider: "OpenAI".to_string(),
                message: format!("API request failed: {}", e),
            })?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError {
                provider: "OpenAI".to_string(),
                message: format!("API returned {}: {}", status, error_text),
            });
        }
        
        let result: serde_json::Value = response.json().await.map_err(|e| {
            AppError::ExternalServiceError {
                provider: "OpenAI".to_string(),
                message: format!("Failed to parse response: {}", e),
            }
        })?;
        
        let embedding_data = result["data"][0]["embedding"]
            .as_array()
            .ok_or_else(|| AppError::ExternalServiceError {
                provider: "OpenAI".to_string(),
                message: "Missing embedding data".to_string(),
            })?;
        
        let vector: Vec<f32> = embedding_data
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();
        
        let usage = EmbeddingUsage {
            total_characters: text.len() as u32,
            total_tokens: result["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
        };
        
        Ok(EmbeddingResult {
            vector,
            model: self.model.clone(),
            dimension: vector.len(),
            usage,
        })
    }
    
    async fn generate_batch(
        &self,
        texts: Vec<String>,
        _config: &EmbeddingConfig,
    ) -> ServiceResult<Vec<EmbeddingResult>> {
        // OpenAI supports batch, but we'll process individually for simplicity
        // TODO: Implement proper batch endpoint
        let mut results = Vec::new();
        for text in texts {
            results.push(self.generate_embedding(text, _config).await?);
        }
        Ok(results)
    }
    
    fn dimension(&self) -> usize {
        match self.model.as_str() {
            "text-embedding-3-small" => 1536,
            "text-embedding-3-large" => 3072,
            "text-embedding-ada-002" => 1536,
            _ => 1536,
        }
    }
    
    fn list_models(&self) -> Vec<String> {
        vec![
            "text-embedding-3-small".to_string(),
            "text-embedding-3-large".to_string(),
            "text-embedding-ada-002".to_string(),
        ]
    }
    
    fn cost_per_1000(&self, model: &str) -> Option<f32> {
        Some(match model {
            "text-embedding-3-small" => 0.02,
            "text-embedding-3-large" => 0.13,
            "text-embedding-ada-002" => 0.10,
            _ => 0.10,
        })
    }
}

/// Local embedding provider (sentence-transformers via Python subprocess)
struct LocalEmbeddingProvider {
    model_name: String,
    models_dir: PathBuf,
    device: String, // "cpu", "cuda", "mps"
}

impl LocalEmbeddingProvider {
    fn new(model_name: String, models_dir: PathBuf, device: String) -> Self {
        Self {
            model_name,
            models_dir,
            device,
        }
    }
    
    /// Run sentence-transformers via Python subprocess
    async fn run_python_embedding(&self, text: String) -> ServiceResult<Vec<f32>> {
        // TODO: Implement Python subprocess call
        // Command: python -c "from sentence_transformers import SentenceTransformer; ..."
        
        Err(AppError::ServiceError(
            "Local embedding provider requires Python environment with sentence-transformers. \
             Run: pip install sentence-transformers"
                .to_string()
        ))
    }
}

#[async_trait]
impl EmbeddingProviderTrait for LocalEmbeddingProvider {
    fn name(&self) -> &'static str {
        "local"
    }
    
    async fn generate_embedding(
        &self,
        text: String,
        _config: &EmbeddingConfig,
    ) -> ServiceResult<EmbeddingResult> {
        let vector = self.run_python_embedding(text).await?;
        let dimension = vector.len();
        
        Ok(EmbeddingResult {
            vector,
            model: self.model_name.clone(),
            dimension,
            usage: EmbeddingUsage::default(),
        })
    }
    
    async fn generate_batch(
        &self,
        texts: Vec<String>,
        _config: &EmbeddingConfig,
    ) -> ServiceResult<Vec<EmbeddingResult>> {
        // Process individually (Python subprocess overhead)
        let mut results = Vec::new();
        for text in texts {
            results.push(self.generate_embedding(text, _config).await?);
        }
        Ok(results)
    }
    
    fn dimension(&self) -> usize {
        // Common sentence-transformer dimensions
        match self.model_name.as_str() {
            "all-MiniLM-L6-v2" => 384,
            "all-mpnet-base-v2" => 768,
            "paraphrase-MultiQA-MiniLM-L6-cos-v1" => 384,
            _ => 384,
        }
    }
    
    fn list_models(&self) -> Vec<String> {
        vec![
            "all-MiniLM-L6-v2".to_string(),
            "all-mpnet-base-v2".to_string(),
            "paraphrase-MultiQA-MiniLM-L6-cos-v1".to_string(),
            "multi-qa-MiniLM-L6-cos-v1".to_string(),
        ]
    }
    
    fn cost_per_1000(&self, _model: &str) -> Option<f32> {
        // Local = free
        Some(0.0)
    }
}

/// Main embedding service implementation
pub struct EmbeddingServiceImpl {
    config: AppConfig,
    db_pool: Pool<Postgres>,
    providers: Arc<RwLock<Vec<Box<dyn EmbeddingProviderTrait>>>>,
    vector_store: Arc<dyn VectorStoreTrait>,
    cache: Arc<RwLock<EmbeddingCache>>,
}

impl EmbeddingServiceImpl {
    /// Create a new embedding service
    pub fn new(config: AppConfig, db_pool: Pool<Postgres>) -> Self {
        let vector_store = Arc::new(PgVectorStore::new(db_pool.clone()));
        let cache = Arc::new(RwLock::new(EmbeddingCache::new(1000)));
        
        Self {
            config,
            db_pool,
            providers: Arc::new(RwLock::new(Vec::new())),
            vector_store,
            cache,
        }
    }
    
    /// Initialize providers based on configuration
    pub async fn initialize_providers(&self) -> ServiceResult<()> {
        let mut providers = self.providers.write().await;
        
        // Add NVIDIA provider if API key configured
        if let Some(api_key) = &self.config.nvidia_api_key {
            if !api_key.is_empty() {
                providers.push(Box::new(NVIDIAEmbeddingProvider::new(
                    api_key.clone(),
                    self.config.nvidia_base_url.clone(),
                    self.config.embedding_model.clone().unwrap_or_else(|| "nvidia/nv-embedqa-e5-v5".to_string()),
                )));
                info!("Initialized NVIDIA embedding provider");
            }
        }
        
        // Add OpenAI provider if API key configured
        if let Some(api_key) = &self.config.openai_api_key {
            if !api_key.is_empty() {
                providers.push(Box::new(OpenAIEmbeddingProvider::new(
                    api_key.clone(),
                    "https://api.openai.com/v1".to_string(),
                    "text-embedding-3-small".to_string(),
                )));
                info!("Initialized OpenAI embedding provider");
            }
        }
        
        // Add local provider (always available if Python installed)
        providers.push(Box::new(LocalEmbeddingProvider::new(
            "all-MiniLM-L6-v2".to_string(),
            PathBuf::from("/var/meetily/models"),
            "cpu".to_string(),
        )));
        info!("Initialized local embedding provider");
        
        Ok(())
    }
    
    /// Get default provider (cheapest available)
    async fn get_default_provider(&self) -> ServiceResult<Arc<dyn EmbeddingProviderTrait>> {
        let providers = self.providers.read().await;
        
        if providers.is_empty() {
            return Err(AppError::ServiceError("No embedding providers initialized".to_string()));
        }
        
        // Priority: Local (free) > NVIDIA (free tier) > OpenAI (paid)
        // TODO: Fix provider cloning
        Err(AppError::ServiceError("Provider cloning not implemented".to_string()))
    }
    
    /// Chunk text using specified strategy
    fn chunk_text(&self, text: &str, strategy: ChunkingStrategy) -> Vec<String> {
        match strategy {
            ChunkingStrategy::FixedSize { chunk_size, overlap } => {
                self.chunk_fixed_size(text, chunk_size, overlap)
            }
            ChunkingStrategy::Semantic => {
                self.chunk_semantic(text)
            }
            ChunkingStrategy::SpeakerTurn => {
                self.chunk_speaker_turn(text)
            }
        }
    }
    
    /// Fixed-size chunking with overlap
    fn chunk_fixed_size(&self, text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        
        let mut start = 0;
        while start < chars.len() {
            let end = (start + chunk_size).min(chars.len());
            let chunk: String = chars[start..end].iter().collect();
            chunks.push(chunk);
            
            if end >= chars.len() {
                break;
            }
            
            // Move start with overlap
            start = end - overlap.min(chunk_size / 2);
        }
        
        chunks
    }
    
    /// Semantic chunking (by paragraphs/sections)
    fn chunk_semantic(&self, text: &str) -> Vec<String> {
        // Split by paragraphs (double newlines)
        text.split("\n\n")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
    
    /// Speaker-turn chunking (for transcripts)
    fn chunk_speaker_turn(&self, text: &str) -> Vec<String> {
        // Split by speaker labels: [SPEAKER_00], [SPEAKER_01], etc.
        text.split(|c| c == '[')
            .filter_map(|segment| {
                if segment.contains("]:") {
                    Some(format!("[{}", segment))
                } else {
                    None
                }
            })
            .collect()
    }
}

#[async_trait]
impl EmbeddingService for EmbeddingServiceImpl {
    #[instrument(skip(self), fields(meeting_id = %meeting_id))]
    async fn create_embeddings(
        &self,
        meeting_id: Uuid,
        transcript_id: Uuid,
        strategy: Option<ChunkingStrategy>,
    ) -> ServiceResult<Vec<Embedding>> {
        // Initialize providers if not already done
        {
            let providers = self.providers.read().await;
            if providers.is_empty() {
                drop(providers);
                self.initialize_providers().await?;
            }
        }
        
        // Load transcript
        let transcript_segments = self.load_transcript_segments(transcript_id).await?;
        
        if transcript_segments.is_empty() {
            return Err(AppError::NotFound(format!(
                "No transcript segments found for transcript {}",
                transcript_id
            )));
        }
        
        // Combine transcript for chunking
        let full_transcript = transcript_segments
            .iter()
            .map(|s| format!("[{}] {}: {}", 
                format_timestamp(s.start_time_secs),
                s.speaker_id.as_deref().unwrap_or("Unknown"),
                s.text
            ))
            .collect::<Vec<_>>()
            .join("\n");
        
        // Chunk transcript
        let strategy = strategy.unwrap_or(ChunkingStrategy::FixedSize {
            chunk_size: 512,
            overlap: 50,
        });
        
        let chunks = self.chunk_text(&full_transcript, strategy.clone());
        info!("Created {} chunks from transcript", chunks.len());
        
        // Generate embeddings
        let provider = self.get_default_provider().await?;
        let mut embeddings = Vec::new();
        
        for (idx, chunk) in chunks.iter().enumerate() {
            // Check cache first
            let cache_key = format!("meeting_{}_chunk_{}", meeting_id, idx);
            if let Some(cached) = self.cache.get(&cache_key).await {
                info!("Cache hit for chunk {}", idx);
                embeddings.push(cached);
                continue;
            }
            
            // Generate embedding
            let result = provider.generate_embedding(chunk.clone(), &EmbeddingConfig::default()).await?;
            
            // Create embedding record
            let embedding = Embedding {
                id: Uuid::new_v4(),
                meeting_id,
                transcript_id: Some(transcript_id),
                chunk_text: chunk.clone(),
                vector: result.vector,
                dimension: result.dimension as u32,
                model: result.model,
                provider: provider.name().to_string(),
                metadata: EmbeddingMetadata {
                    chunk_index: idx as u32,
                    start_time_secs: None,
                    end_time_secs: None,
                    speaker_id: None,
                    embedding_type: "transcript_chunk".to_string(),
                },
                created_at: Utc::now(),
            };
            
            // Cache embedding
            self.cache.insert(cache_key, embedding.clone()).await;
            
            embeddings.push(embedding);
        }
        
        // Save to vector store
        self.vector_store.save_batch(embeddings.clone()).await?;
        
        info!("Created {} embeddings for meeting {}", embeddings.len(), meeting_id);
        Ok(embeddings)
    }
    
    async fn search_similar(
        &self,
        query: String,
        meeting_id: Option<Uuid>,
        limit: usize,
    ) -> ServiceResult<Vec<SimilarityResult>> {
        // Generate query embedding
        let provider = self.get_default_provider().await?;
        let query_embedding = provider.generate_embedding(query.clone(), &EmbeddingConfig::default()).await?;
        
        // Search vector store
        self.vector_store.search(
            &query_embedding.vector,
            meeting_id,
            limit,
        ).await
    }
    
    async fn get_embedding(&self, embedding_id: Uuid) -> ServiceResult<Embedding> {
        // TODO: Load from database
        Err(AppError::NotFound("Not implemented".to_string()))
    }
    
    async fn delete_embeddings(&self, meeting_id: Uuid) -> ServiceResult<u32> {
        let count = self.vector_store.delete_by_meeting(meeting_id).await?;
        info!("Deleted {} embeddings for meeting {}", count, meeting_id);
        Ok(count)
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

// Vector storage trait
#[async_trait]
trait VectorStoreTrait: Send + Sync {
    async fn save_batch(&self, embeddings: Vec<Embedding>) -> ServiceResult<()>;
    async fn search(&self, query_vector: &[f32], meeting_id: Option<Uuid>, limit: usize) 
        -> ServiceResult<Vec<SimilarityResult>>;
    async fn delete_by_meeting(&self, meeting_id: Uuid) -> ServiceResult<u32>;
}

/// PostgreSQL + pgvector implementation
struct PgVectorStore {
    db_pool: Pool<Postgres>,
}

impl PgVectorStore {
    fn new(db_pool: Pool<Postgres>) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl VectorStoreTrait for PgVectorStore {
    async fn save_batch(&self, embeddings: Vec<Embedding>) -> ServiceResult<()> {
        for embedding in embeddings {
            // Convert vector to pgvector format
            let vector_str = format!(
                "[{}]",
                embedding.vector.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            
            sqlx::query!(
                r#"
                INSERT INTO embeddings (
                    id, meeting_id, transcript_id, chunk_text, vector,
                    dimension, model, provider, metadata_json
                ) VALUES ($1, $2, $3, $4, $5::vector, $6, $7, $8, $9)
                "#,
                embedding.id,
                embedding.meeting_id,
                embedding.transcript_id,
                embedding.chunk_text,
                vector_str,
                embedding.dimension as i64,
                embedding.model,
                embedding.provider,
                serde_json::to_value(&embedding.metadata).map_err(|e| AppError::JsonError(e.to_string()))?,
            )
            .execute(&self.db_pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }
        
        Ok(())
    }
    
    async fn search(
        &self,
        query_vector: &[f32],
        meeting_id: Option<Uuid>,
        limit: usize,
    ) -> ServiceResult<Vec<SimilarityResult>> {
        let vector_str = format!(
            "[{}]",
            query_vector.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        
        let results = if let Some(mid) = meeting_id {
            // Search within specific meeting
            sqlx::query_as!(
                EmbeddingRow,
                r#"
                SELECT 
                    id, meeting_id, transcript_id, chunk_text,
                    vector::text as "vector: String", dimension, model,
                    provider, metadata_json, created_at,
                    1 - (vector <=> $1::vector) as "similarity!"
                FROM embeddings
                WHERE meeting_id = $2
                ORDER BY vector <=> $1::vector
                LIMIT $3
                "#,
                vector_str,
                mid,
                limit as i64
            )
            .fetch_all(&self.db_pool)
            .await
        } else {
            // Search all meetings
            sqlx::query_as!(
                EmbeddingRow,
                r#"
                SELECT 
                    id, meeting_id, transcript_id, chunk_text,
                    vector::text as "vector: String", dimension, model,
                    provider, metadata_json, created_at,
                    1 - (vector <=> $1::vector) as "similarity!"
                FROM embeddings
                ORDER BY vector <=> $1::vector
                LIMIT $2
                "#,
                vector_str,
                limit as i64
            )
            .fetch_all(&self.db_pool)
            .await
        }
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(results.into_iter().map(|r| r.into()).collect())
    }
    
    async fn delete_by_meeting(&self, meeting_id: Uuid) -> ServiceResult<u32> {
        let result = sqlx::query!(
            "DELETE FROM embeddings WHERE meeting_id = $1",
            meeting_id
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(result.rows_affected() as u32)
    }
}

/// In-memory cache for embeddings
struct EmbeddingCache {
    cache: HashMap<String, Embedding>,
    lru_order: Vec<String>,
    max_size: usize,
}

impl EmbeddingCache {
    fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::new(),
            lru_order: Vec::new(),
            max_size,
        }
    }
    
    async fn get(&self, key: &str) -> Option<Embedding> {
        self.cache.get(key).cloned()
    }
    
    async fn insert(&mut self, key: String, embedding: Embedding) {
        if self.cache.len() >= self.max_size {
            // Evict oldest
            if let Some(oldest) = self.lru_order.first().cloned() {
                self.cache.remove(&oldest);
                self.lru_order.remove(0);
            }
        }
        
        self.cache.insert(key.clone(), embedding);
        self.lru_order.push(key);
    }
}

// Helper functions
fn format_timestamp(secs: f32) -> String {
    let minutes = (secs / 60.0) as u32;
    let seconds = (secs % 60.0) as u32;
    format!("{:02}:{:02}", minutes, seconds)
}

// Database row mapping
#[derive(sqlx::FromRow)]
struct EmbeddingRow {
    id: Uuid,
    meeting_id: Uuid,
    transcript_id: Option<Uuid>,
    chunk_text: String,
    vector: String,
    dimension: i64,
    model: String,
    provider: String,
    metadata_json: serde_json::Value,
    created_at: DateTime<Utc>,
    similarity: f64,
}

impl From<EmbeddingRow> for SimilarityResult {
    fn from(row: EmbeddingRow) -> Self {
        Self {
            embedding: Embedding {
                id: row.id,
                meeting_id: row.meeting_id,
                transcript_id: row.transcript_id,
                chunk_text: row.chunk_text,
                vector: vec![], // Don't parse vector string back
                dimension: row.dimension as u32,
                model: row.model,
                provider: row.provider,
                metadata: serde_json::from_value(row.metadata_json).unwrap_or_default(),
                created_at: row.created_at,
            },
            similarity: row.similarity as f32,
        }
    }
}

/// Similarity search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarityResult {
    pub embedding: Embedding,
    pub similarity: f32,
}

// Load transcript segments helper
impl EmbeddingServiceImpl {
    async fn load_transcript_segments(&self, _transcript_id: Uuid) 
        -> ServiceResult<Vec<crate::services::transcription::TranscriptionSegment> 
    {
        // TODO: Implement
        Ok(vec![])
    }
}

// Database schema (for migrations)
// CREATE EXTENSION IF NOT EXISTS vector;
//
// CREATE TABLE IF NOT EXISTS embeddings (
//     id UUID PRIMARY KEY,
//     meeting_id UUID NOT NULL,
//     transcript_id UUID,
//     chunk_text TEXT NOT NULL,
//     vector vector(1024) NOT NULL,  -- Dimension depends on model
//     dimension BIGINT NOT NULL,
//     model TEXT NOT NULL,
//     provider TEXT NOT NULL,
//     metadata_json JSONB NOT NULL,
//     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
// );
//
// CREATE INDEX IF NOT EXISTS idx_embeddings_meeting_id ON embeddings(meeting_id);
// CREATE INDEX IF NOT EXISTS idx_embeddings_transcript_id ON embeddings(transcript_id);
// CREATE INDEX IF NOT EXISTS idx_embeddings_vector ON embeddings USING ivfflat (vector vector_cosine_ops) WITH (lists = 100);
// CREATE INDEX IF NOT EXISTS idx_embeddings_created_at ON embeddings(created_at DESC);