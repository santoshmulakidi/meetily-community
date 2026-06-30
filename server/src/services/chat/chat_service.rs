//! Chat service implementation
//!
//! Features:
//! - ChatGPT-style conversation over meetings
//! - RAG (Retrieval-Augmented Generation) pipeline
//! - Conversation memory (chat history)
//! - Multi-meeting context
//! - Citation generation (link to transcript timestamps)
//! - Streaming responses

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use tracing::{info, warn, error, debug, instrument};

use crate::error::{AppError, ServiceResult};
use super::{
    ChatService, ChatMessage, ChatRole, Conversation, ChatConfig,
    ChatResponse, Citation,
};
use crate::config::ChatConfig as AppConfig;

/// Main chat service implementation
pub struct ChatServiceImpl {
    config: AppConfig,
    db_pool: Pool<Postgres>,
    conversations: Arc<RwLock<HashMap<Uuid, Conversation>>>,  // conversation_id -> conversation
    llm_client: Arc<dyn LLMClientTrait>,
    embedding_client: Arc<dyn EmbeddingClientTrait>,
}

impl ChatServiceImpl {
    /// Create a new chat service
    pub fn new(config: AppConfig, db_pool: Pool<Postgres>) -> Self {
        // TODO: Initialize LLM and embedding clients from config
        // For now, use placeholders
        
        Self {
            config,
            db_pool,
            conversations: Arc::new(RwLock::new(HashMap::new())),
            llm_client: Arc::new(PlaceholderLLMClient),
            embedding_client: Arc::new(PlaceholderEmbeddingClient),
        }
    }
    
    /// Retrieve relevant context from meetings using RAG
    async fn retrieve_context(
        &self,
        query: String,
        meeting_ids: Option<Vec<Uuid>>,
        max_chunks: usize,
    ) -> ServiceResult<Vec<ContextChunk>> {
        // Generate query embedding
        let query_embedding = self.embedding_client.generate_embedding(query.clone()).await?;
        
        // Search embeddings in database
        let mut context_chunks = Vec::new();
        
        if let Some(meeting_ids) = meeting_ids {
            // Search within specific meetings
            for meeting_id in meeting_ids {
                let chunks = self.search_meeting_embeddings(meeting_id, &query_embedding, max_chunks / meeting_ids.len()).await?;
                context_chunks.extend(chunks);
            }
        } else {
            // Search all meetings
            context_chunks = self.search_all_embeddings(&query_embedding, max_chunks).await?;
        }
        
        info!("Retrieved {} context chunks for query", context_chunks.len());
        Ok(context_chunks)
    }
    
    /// Search embeddings within a specific meeting
    async fn search_meeting_embeddings(
        &self,
        meeting_id: Uuid,
        query_embedding: &[f32],
        limit: usize,
    ) -> ServiceResult<Vec<ContextChunk>> {
        let vector_str = format!(
            "[{}]",
            query_embedding.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        
        let results = sqlx::query_as!(
            EmbeddingSearchRow,
            r#"
            SELECT 
                e.id,
                e.meeting_id,
                e.transcript_id,
                e.chunk_text,
                e.metadata_json,
                1 - (e.vector <=> $1::vector) as "similarity!"
            FROM embeddings e
            WHERE e.meeting_id = $2
            ORDER BY e.vector <=> $1::vector
            LIMIT $3
            "#,
            vector_str,
            meeting_id,
            limit as i64
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(results.into_iter().map(|r| r.into()).collect())
    }
    
    /// Search embeddings across all meetings
    async fn search_all_embeddings(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> ServiceResult<Vec<ContextChunk>> {
        let vector_str = format!(
            "[{}]",
            query_embedding.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        
        let results = sqlx::query_as!(
            EmbeddingSearchRow,
            r#"
            SELECT 
                e.id,
                e.meeting_id,
                e.transcript_id,
                e.chunk_text,
                e.metadata_json,
                1 - (e.vector <=> $1::vector) as "similarity!"
            FROM embeddings e
            ORDER BY e.vector <=> $1::vector
            LIMIT $2
            "#,
            vector_str,
            limit as i64
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(results.into_iter().map(|r| r.into()).collect())
    }
    
    /// Build prompt with retrieved context
    fn build_rag_prompt(
        &self,
        query: &str,
        context: &[ContextChunk],
        conversation_history: &[ChatMessage],
    ) -> String {
        let system_prompt = r#"You are an intelligent meeting assistant with access to meeting transcripts and summaries.
Your task is to answer questions based on the provided meeting context.

Guidelines:
- Answer based ONLY on the provided context
- If the answer isn't in the context, say so clearly
- Cite specific meetings and timestamps when possible
- Be concise but thorough
- Use bullet points for lists
- Include direct quotes when relevant

Format your answer with citations like:
- Statement [Meeting: ABC Corp, 15:30-16:00, Speaker: Alice]
"#;

        // Build context section
        let context_text = context
            .iter()
            .enumerate()
            .map(|(i, chunk)| {
                format!(
                    "[Context {}]\nMeeting: {}\nSpeaker: {}\nTime: {} - {}\nText: {}\n",
                    i + 1,
                    chunk.meeting_name.as_deref().unwrap_or("Unknown"),
                    chunk.speaker_id.as_deref().unwrap_or("Unknown"),
                    format_timestamp(chunk.start_time_secs),
                    format_timestamp(chunk.end_time_secs),
                    chunk.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Build conversation history
        let history_text = conversation_history
            .iter()
            .map(|msg| {
                format!(
                    "{}: {}",
                    match msg.role {
                        ChatRole::User => "User",
                        ChatRole::Assistant => "Assistant",
                    },
                    msg.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "{}\n\n=== CONTEXT ===\n{}\n\n=== CONVERSATION HISTORY ===\n{}\n\n=== USER QUESTION ===\n{}\n\nAssistant:",
            system_prompt, context_text, history_text, query
        )
    }
    
    /// Generate citations from context chunks
    fn generate_citations(&self, context: &[ContextChunk], answer: &str) -> Vec<Citation> {
        // Simple heuristic: extract mentions of context in answer
        // In production, use more sophisticated attribution
        
        context
            .iter()
            .map(|chunk| Citation {
                meeting_id: chunk.meeting_id,
                transcript_id: chunk.transcript_id,
                start_time_secs: chunk.start_time_secs,
                end_time_secs: chunk.end_time_secs,
                speaker_id: chunk.speaker_id.clone(),
                text: chunk.text.clone(),
                relevance_score: chunk.similarity,
            })
            .collect()
    }
}

#[async_trait]
impl ChatService for ChatServiceImpl {
    #[instrument(skip(self), fields(conversation_id = ?conversation_id))]
    async fn chat(
        &self,
        conversation_id: Option<Uuid>,
        meeting_ids: Option<Vec<Uuid>>,
        message: String,
        config: Option<ChatConfig>,
    ) -> ServiceResult<ChatResponse> {
        // Get or create conversation
        let conversation_id = conversation_id.unwrap_or_else(Uuid::new_v4);
        
        let mut conversations = self.conversations.write().await;
        let conversation = conversations
            .entry(conversation_id)
            .or_insert_with(|| Conversation {
                id: conversation_id,
                meeting_ids: meeting_ids.clone().unwrap_or_default(),
                messages: Vec::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            });
        
        // Add user message to history
        let user_message = ChatMessage {
            role: ChatRole::User,
            content: message.clone(),
            timestamp: Utc::now(),
        };
        conversation.messages.push(user_message);
        
        // Retrieve context using RAG
        let config = config.unwrap_or_default();
        let context = self.retrieve_context(
            message.clone(),
            meeting_ids,
            config.max_context_chunks,
        ).await?;
        
        // Build prompt with RAG
        let prompt = self.build_rag_prompt(
            &message,
            &context,
            &conversation.messages,
        );
        
        // Generate response using LLM
        let assistant_text = self.llm_client.generate_completion(prompt).await?;
        
        // Generate citations
        let citations = self.generate_citations(&context, &assistant_text);
        
        // Add assistant message to history
        let assistant_message = ChatMessage {
            role: ChatRole::Assistant,
            content: assistant_text.clone(),
            timestamp: Utc::now(),
        };
        conversation.messages.push(assistant_message);
        conversation.updated_at = Utc::now();
        
        // Save conversation to database (async, non-blocking)
        let db_pool_clone = self.db_pool.clone();
        let conv_clone = conversation.clone();
        tokio::spawn(async move {
            let _ = save_conversation_to_db(&db_pool_clone, &conv_clone).await;
        });
        
        info!(
            "Chat response generated: {} chars, {} citations",
            assistant_text.len(),
            citations.len()
        );
        
        Ok(ChatResponse {
            conversation_id,
            message: assistant_text,
            citations,
            context_used: context.len(),
            model_used: "placeholder".to_string(),
            created_at: Utc::now(),
        })
    }
    
    async fn get_conversation(&self, conversation_id: Uuid) -> ServiceResult<Conversation> {
        let conversations = self.conversations.read().await;
        
        conversations
            .get(&conversation_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Conversation {} not found", conversation_id)))
    }
    
    async fn list_conversations(&self, limit: usize) -> ServiceResult<Vec<Conversation>> {
        let conversations = self.conversations.read().await;
        
        let mut convs: Vec<_> = conversations.values().cloned().collect();
        convs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        
        Ok(convs.into_iter().take(limit).collect())
    }
    
    async fn delete_conversation(&self, conversation_id: Uuid) -> ServiceResult<()> {
        let mut conversations = self.conversations.write().await;
        
        conversations.remove(&conversation_id);
        
        // Also delete from database
        sqlx::query!("DELETE FROM conversations WHERE id = $1", conversation_id)
            .execute(&self.db_pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        info!("Deleted conversation {}", conversation_id);
        Ok(())
    }
    
    async fn clear_conversation_history(&self, conversation_id: Uuid) -> ServiceResult<()> {
        let mut conversations = self.conversations.write().await;
        
        if let Some(conversation) = conversations.get_mut(&conversation_id) {
            conversation.messages.clear();
            conversation.updated_at = Utc::now();
        }
        
        Ok(())
    }
}

// LLM Client trait
#[async_trait]
trait LLMClientTrait: Send + Sync {
    async fn generate_completion(&self, prompt: String) -> ServiceResult<String>;
}

// Placeholder LLM client (to be replaced with actual implementation)
struct PlaceholderLLMClient;

#[async_trait]
impl LLMClientTrait for PlaceholderLLMClient {
    async fn generate_completion(&self, prompt: String) -> ServiceResult<String> {
        // Return placeholder response
        Ok(format!("This is a placeholder response. In production, this would call an LLM with the following prompt ({} chars):\n\n{}", prompt.len(), &prompt[..prompt.len().min(500)]))
    }
}

// Embedding Client trait
#[async_trait]
trait EmbeddingClientTrait: Send + Sync {
    async fn generate_embedding(&self, text: String) -> ServiceResult<Vec<f32>>;
}

// Placeholder embedding client
struct PlaceholderEmbeddingClient;

#[async_trait]
impl EmbeddingClientTrait for PlaceholderEmbeddingClient {
    async fn generate_embedding(&self, _text: String) -> ServiceResult<Vec<f32>> {
        // Return dummy embedding
        Ok(vec![0.0; 1024])
    }
}

// Database helper
async fn save_conversation_to_db(
    db_pool: &Pool<Postgres>,
    conversation: &Conversation,
) -> ServiceResult<()> {
    // Save conversation metadata
    sqlx::query!(
        r#"
        INSERT INTO conversations (
            id, meeting_ids_json, created_at, updated_at
        ) VALUES ($1, $2, $3, $4)
        ON CONFLICT (id) DO UPDATE SET
            meeting_ids_json = EXCLUDED.meeting_ids_json,
            updated_at = EXCLUDED.updated_at
        "#,
        conversation.id,
        serde_json::to_value(&conversation.meeting_ids).map_err(|e| AppError::JsonError(e.to_string()))?,
        conversation.created_at,
        conversation.updated_at,
    )
    .execute(db_pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    
    // Save messages
    for msg in &conversation.messages {
        sqlx::query!(
            r#"
            INSERT INTO conversation_messages (
                conversation_id, role, content, created_at
            ) VALUES ($1, $2, $3, $4)
            "#,
            conversation.id,
            match msg.role {
                ChatRole::User => "user",
                ChatRole::Assistant => "assistant",
            },
            &msg.content,
            msg.timestamp,
        )
        .execute(db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    }
    
    Ok(())
}

// Context chunk from RAG retrieval
#[derive(Debug, Clone)]
struct ContextChunk {
    id: Uuid,
    meeting_id: Uuid,
    transcript_id: Option<Uuid>,
    text: String,
    speaker_id: Option<String>,
    start_time_secs: f32,
    end_time_secs: f32,
    meeting_name: Option<String>,
    similarity: f32,
}

// Database row for embedding search
#[derive(sqlx::FromRow)]
struct EmbeddingSearchRow {
    id: Uuid,
    meeting_id: Uuid,
    transcript_id: Option<Uuid>,
    chunk_text: String,
    metadata_json: serde_json::Value,
    similarity: f32,
}

impl From<EmbeddingSearchRow> for ContextChunk {
    fn from(row: EmbeddingSearchRow) -> Self {
        // Parse metadata from JSON
        let metadata: serde_json::Value = row.metadata_json;
        let speaker_id = metadata["speaker_id"].as_str().map(String::from);
        let start_time_secs = metadata["start_time_secs"].as_f64().unwrap_or(0.0) as f32;
        let end_time_secs = metadata["end_time_secs"].as_f64().unwrap_or(0.0) as f32;
        
        Self {
            id: row.id,
            meeting_id: row.meeting_id,
            transcript_id: row.transcript_id,
            text: row.chunk_text,
            speaker_id,
            start_time_secs,
            end_time_secs,
            meeting_name: None, // TODO: Join with meetings table
            similarity: row.similarity,
        }
    }
}

// Helper function
fn format_timestamp(secs: f32) -> String {
    let minutes = (secs / 60.0) as u32;
    let seconds = (secs % 60.0) as u32;
    format!("{:02}:{:02}", minutes, seconds)
}

// Database schema (for migrations)
// CREATE TABLE IF NOT EXISTS conversations (
//     id UUID PRIMARY KEY,
//     meeting_ids_json JSONB NOT NULL,  // Array of meeting IDs
//     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
//     updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
// );
//
// CREATE TABLE IF NOT EXISTS conversation_messages (
//     id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
//     conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
//     role TEXT NOT NULL,  -- 'user' or 'assistant'
//     content TEXT NOT NULL,
//     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
// );
//
// CREATE INDEX IF NOT EXISTS idx_conversations_updated_at ON conversations(updated_at DESC);
// CREATE INDEX IF NOT EXISTS idx_conversation_messages_conversation_id ON conversation_messages(conversation_id);