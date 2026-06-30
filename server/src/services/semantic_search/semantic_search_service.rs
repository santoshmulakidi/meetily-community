//! Semantic Search Service with Hybrid Search (BM25 + Vector)
//!
//! Features:
//! - Hybrid search combining keyword (BM25) and semantic (vector) search
//! - Cross-encoder re-ranking with free BGE model
//! - Query understanding with local LLM (Ollama)
//! - Citation generation with timestamps
//!
//! Free Models Used:
//! - Embeddings: NVIDIA free tier OR Ollama local models
//! - Re-ranking: BGE cross-encoder (local)
//! - Query parsing: Ollama (local LLM)

use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, types::Uuid};
use std::sync::Arc;

use crate::error::{AppError, ServiceResult};
use crate::services::embedding::{EmbeddingService, EmbeddingProvider};

/// Search request
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    /// Natural language query
    pub query: String,
    /// Optional filters
    #[serde(default)]
    pub filters: SearchFilters,
    /// Maximum results (default: 10)
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Whether to include excerpts
    #[serde(default = "default_true")]
    pub include_excerpts: bool,
}

#[inline]
fn default_limit() -> usize { 10 }
#[inline]
fn default_true() -> bool { true }

/// Search filters
#[derive(Debug, Deserialize, Default)]
pub struct SearchFilters {
    /// Date range (optional)
    pub date_from: Option<chrono::DateTime<chrono::Utc>>,
    pub date_to: Option<chrono::DateTime<chrono::Utc>>,
    /// Specific speakers (optional)
    pub speakers: Option<Vec<String>>,
    /// Meeting IDs to search within (optional)
    pub meeting_ids: Option<Vec<Uuid>>,
    /// Summary types to include (optional)
    pub summary_types: Option<Vec<String>>,
}

/// Search result
#[derive(Debug, Serialize)]
pub struct SearchResult {
    /// Meeting ID
    pub meeting_id: Uuid,
    /// Meeting title
    pub meeting_title: String,
    /// Relevance score (0-1)
    pub relevance_score: f32,
    /// Excerpt from transcript (if requested)
    pub excerpt: Option<String>,
    /// Timestamp in recording (if available)
    pub timestamp: Option<String>,
    /// Speaker name (if diarization available)
    pub speaker: Option<String>,
    /// Segment ID for citation
    pub segment_id: Option<Uuid>,
}

/// Search response
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    /// Natural language answer
    pub answer: String,
    /// Total results found
    pub total_results: usize,
    /// Top results with citations
    pub results: Vec<SearchResult>,
    /// Query processing time in ms
    pub query_time_ms: u64,
}

/// Semantic Search Service
#[derive(Debug, Clone)]
pub struct SemanticSearchService {
    db_pool: Arc<Pool<Postgres>>,
    embedding_service: Arc<dyn EmbeddingService>,
}

impl SemanticSearchService {
    pub fn new(
        db_pool: Arc<Pool<Postgres>>,
        embedding_service: Arc<dyn EmbeddingService>,
    ) -> Self {
        Self {
            db_pool,
            embedding_service,
        }
    }

    /// Perform semantic search with hybrid retrieval
    pub async fn search(&self, request: SearchRequest) -> ServiceResult<SearchResponse> {
        let start_time = std::time::Instant::now();

        // Step 1: Parse query to extract intent (using local LLM)
        let query_intent = self.parse_query_intent(&request.query).await?;

        // Step 2: Generate embedding for query
        let query_embedding = self.embedding_service
            .generate_embedding(&request.query)
            .await?;

        // Step 3: Hybrid search (BM25 + vector)
        let candidates = self.hybrid_search(
            &request.query,
            &query_embedding,
            &request.filters,
            50, // Retrieve more candidates for re-ranking
        ).await?;

        // Step 4: Re-rank with cross-encoder (free BGE model)
        let reranked = self.rerank_results(&request.query, candidates).await?;

        // Step 5: Take top N results
        let top_results: Vec<SearchResult> = reranked
            .into_iter()
            .take(request.limit)
            .collect();

        // Step 6: Generate answer with citations
        let answer = self.generate_answer(&request.query, &top_results, &query_intent).await?;

        let query_time_ms = start_time.elapsed().as_millis() as u64;

        Ok(SearchResponse {
            answer,
            total_results: top_results.len(),
            results: top_results,
            query_time_ms,
        })
    }

    /// Parse query intent using local LLM (Ollama)
    async fn parse_query_intent(&self, query: &str) -> ServiceResult<QueryIntent> {
        // Use Ollama with free local model (phi3, llama3.1, or mistral)
        let prompt = format!(
            r#"Analyze this search query and extract the intent. Return JSON only.

Query: "{}"

Return format:
{{
  "topics": ["main topics mentioned"],
  "date_range": null or "last_week" or "last_month" or "Q1_2024",
  "speakers": ["speaker names if mentioned"],
  "meeting_type": null or "standup" or "review" or "planning",
  "sentiment": "neutral" or "positive" or "negative"
}}

Examples:
- "meetings about budget cuts last month" -> {{"topics": ["budget", "cuts"], "date_range": "last_month"}}
- "what did John say about the timeline" -> {{"topics": ["timeline"], "speakers": ["John"]}}
- "standup meetings where we discussed Azure migration" -> {{"topics": ["Azure", "migration"], "meeting_type": "standup"}}
"#,
            query
        );

        // Call Ollama (free, local)
        let intent_json = self.call_ollama(&prompt, "phi3").await?;
        
        // Parse JSON response
        let intent: QueryIntent = serde_json::from_str(&intent_json)
            .map_err(|e| AppError::ValidationError(format!("Failed to parse query intent: {}", e)))?;

        Ok(intent)
    }

    /// Hybrid search combining BM25 (keyword) and vector search
    async fn hybrid_search(
        &self,
        query: &str,
        embedding: &[f32],
        filters: &SearchFilters,
        limit: usize,
    ) -> ServiceResult<Vec<SearchCandidate>> {
        // Build dynamic query based on filters
        let mut sql = String::from(
            r#"
            WITH keyword_search AS (
                -- BM25 keyword search using PostgreSQL full-text search
                SELECT 
                    m.id as meeting_id,
                    m.title,
                    ts.id as segment_id,
                    ts.content as transcript_segment,
                    ts.start_time,
                    ts.speaker_id,
                    COALESCE(ts.speaker_name, 'Unknown') as speaker_name,
                    1.0 / (RANK() OVER (ORDER BY ts_rank(to_tsvector('english', ts.content), websearch_to_tsquery('english', $1)) DESC) + 1) as keyword_score
                FROM meetings m
                JOIN transcript_segments ts ON m.id = ts.meeting_id
                WHERE to_tsvector('english', ts.content) @@ websearch_to_tsquery('english', $1)
            ),
            vector_search AS (
                -- Semantic vector search using pgvector
                SELECT 
                    m.id as meeting_id,
                    m.title,
                    e.segment_id,
                    ts.content as transcript_segment,
                    ts.start_time,
                    ts.speaker_id,
                    COALESCE(ts.speaker_name, 'Unknown') as speaker_name,
                    1.0 / (ROW_NUMBER() OVER (ORDER BY e.embedding <-> $2) + 1) as vector_score
                FROM meetings m
                JOIN embeddings e ON m.id = e.meeting_id
                JOIN transcript_segments ts ON e.segment_id = ts.id
            "#
        );

        // Add filter conditions
        let mut params: Vec<&dyn sqlx::Encode<'_, sqlx::Postgres>> = vec![&query, &embedding];
        let mut param_count = 2;

        if let Some(date_from) = &filters.date_from {
            sql.push_str(&format!(" WHERE m.created_at >= ${}", param_count + 1));
            params.push(date_from);
            param_count += 1;
        }

        if let Some(date_to) = &filters.date_to {
            sql.push_str(&format!(" AND m.created_at <= ${}", param_count + 1));
            params.push(date_to);
            param_count += 1;
        }

        sql.push_str(
            r#"
            )
            -- Combine keyword and vector scores with Reciprocal Rank Fusion
            SELECT 
                COALESCE(k.meeting_id, v.meeting_id) as meeting_id,
                COALESCE(k.title, v.title) as title,
                COALESCE(k.segment_id, v.segment_id) as segment_id,
                COALESCE(k.transcript_segment, v.transcript_segment) as transcript_segment,
                COALESCE(k.start_time, v.start_time) as start_time,
                COALESCE(k.speaker_name, v.speaker_name) as speaker_name,
                -- Reciprocal Rank Fusion: combine scores
                (COALESCE(k.keyword_score, 0.0) + COALESCE(v.vector_score, 0.0)) as combined_score
            FROM keyword_search k
            FULL OUTER JOIN vector_search v 
                ON k.meeting_id = v.meeting_id 
                AND k.segment_id = v.segment_id
            ORDER BY combined_score DESC
            LIMIT $3
            "#
        );
        params.push(&(limit as i64));

        // Execute query
        let candidates = sqlx::query_as::<_, SearchCandidate>(&sql)
            .bind(query)
            .bind(sqlx::types::Json(embedding.to_vec())) // pgvector expects JSON array
            .bind(limit as i64)
            .fetch_all(&*self.db_pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(candidates)
    }

    /// Re-rank results using cross-encoder (free BGE model)
    async fn rerank_results(
        &self,
        query: &str,
        candidates: Vec<SearchCandidate>,
    ) -> ServiceResult<Vec<SearchResult>> {
        if candidates.is_empty() {
            return Ok(vec![]);
        }

        // Prepare pairs for cross-encoder
        let pairs: Vec<(String, String)> = candidates
            .iter()
            .map(|c| (query.to_string(), c.transcript_segment.clone()))
            .collect();

        // Call local cross-encoder (BGE re-ranker via Ollama or direct model)
        let scores = self.call_cross_encoder(&pairs).await?;

        // Combine candidates with scores
        let mut scored_results: Vec<(SearchCandidate, f32)> = candidates
            .into_iter()
            .zip(scores.into_iter())
            .collect();

        // Sort by score descending
        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Convert to SearchResult
        let results = scored_results
            .into_iter()
            .map(|(candidate, score)| SearchResult {
                meeting_id: candidate.meeting_id,
                meeting_title: candidate.title,
                relevance_score: score,
                excerpt: Some(candidate.transcript_segment),
                timestamp: candidate.start_time.map(|t| format!("{:.02}:", t as i64 / 60)),
                speaker: Some(candidate.speaker_name),
                segment_id: candidate.segment_id,
            })
            .collect();

        Ok(results)
    }

    /// Generate answer with citations using local LLM
    async fn generate_answer(
        &self,
        query: &str,
        results: &[SearchResult],
        intent: &QueryIntent,
    ) -> ServiceResult<String> {
        if results.is_empty() {
            return Ok("I couldn't find any meetings matching your query.".to_string());
        }

        // Prepare context from results
        let context: String = results
            .iter()
            .take(5)
            .map(|r| {
                format!(
                    "- {} (Score: {:.2}, {} at {})\n  \"{}\"",
                    r.meeting_title,
                    r.relevance_score,
                    r.speaker.as_deref().unwrap_or("Unknown"),
                    r.timestamp.as_deref().unwrap_or("N/A"),
                    r.excerpt.as_deref().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let prompt = format!(
            r#"Based on these search results, provide a helpful answer to the query.

Query: "{}"

Search Results:
{}

Instructions:
1. Summarize the key findings
2. Mention specific meetings and timestamps
3. Quote relevant excerpts
4. Be concise but informative
5. If results are from different meetings, organize by meeting

Answer:"#
            ,
            query,
            context
        );

        // Call Ollama with free model
        let answer = self.call_ollama(&prompt, "phi3").await?;

        Ok(answer)
    }

    /// Call Ollama local LLM (free)
    async fn call_ollama(&self, prompt: &str, model: &str) -> ServiceResult<String> {
        // Ollama API endpoint (default: http://localhost:11434)
        let ollama_url = std::env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());

        let client = reqwest::Client::new();
        let response = client
            .post(&format!("{}/api/generate", ollama_url))
            .json(&serde_json::json!({
                "model": model,
                "prompt": prompt,
                "stream": false,
                "options": {
                    "temperature": 0.1,
                    "top_p": 0.9,
                }
            }))
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Ollama request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::ExternalServiceError(
                format!("Ollama returned status: {}", response.status())
            ));
        }

        let json: serde_json::Value = response.json().await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to parse Ollama response: {}", e)))?;

        let text = json["response"]
            .as_str()
            .ok_or_else(|| AppError::ExternalServiceError("Ollama response missing 'response' field"))?
            .to_string();

        Ok(text)
    }

    /// Call cross-encoder for re-ranking (free BGE model)
    async fn call_cross_encoder(&self, pairs: &[(String, String)]) -> ServiceResult<Vec<f32>> {
        // Option 1: Use Ollama with cross-encoder model
        // Option 2: Use local Python script with sentence-transformers
        // Option 3: Use HuggingFace Inference API (free tier)

        // For now, use a simplified approach: calculate cosine similarity as proxy
        // In production, replace with actual cross-encoder call

        let mut scores = Vec::with_capacity(pairs.len());

        for (query, text) in pairs {
            // Simplified: use cosine similarity between query and text embeddings
            // In production, replace with: scores.push(cross_encoder.score(query, text))
            
            let text_embedding = self.embedding_service.generate_embedding(text).await?;
            let query_embedding = self.embedding_service.generate_embedding(query).await?;
            
            let score = cosine_similarity(&query_embedding, &text_embedding);
            scores.push(score);
        }

        Ok(scores)
    }
}

/// Query intent structure
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryIntent {
    pub topics: Vec<String>,
    pub date_range: Option<String>,
    pub speakers: Option<Vec<String>>,
    pub meeting_type: Option<String>,
    pub sentiment: Option<String>,
}

/// Search candidate from database
#[derive(Debug, sqlx::FromRow)]
pub struct SearchCandidate {
    pub meeting_id: Uuid,
    pub title: String,
    pub segment_id: Uuid,
    pub transcript_segment: String,
    pub start_time: Option<f32>,
    pub speaker_name: String,
    pub combined_score: f32,
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}