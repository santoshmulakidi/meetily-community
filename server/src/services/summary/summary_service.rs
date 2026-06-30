//! Summary service implementation
//!
//! Features:
//! - Pluggable LLM provider architecture (OpenRouter, Ollama, NVIDIA, OpenAI-compatible)
//! - Multiple summary types (executive, technical, action items, decisions, risks)
//! - Custom prompt templates
//! - Streaming support for long meetings
//! - Cost optimization (cheapest provider first)

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
use tokio_stream::Stream;
use std::pin::Pin;

use crate::error::{AppError, ServiceResult};
use super::{
    SummaryService, Summary, SummaryType, SummarySegment,
    SummaryProvider, SummaryConfig, SummaryMetadata,
};
use crate::config::SummaryConfig as AppConfig;

/// LLM Provider trait - all summary backends must implement this
#[async_trait]
trait LLMProviderTrait: Send + Sync {
    /// Get provider name
    fn name(&self) -> &'static str;
    
    /// Generate a completion (non-streaming)
    async fn generate_completion(
        &self,
        prompt: String,
        config: &LLMConfig,
    ) -> ServiceResult<LLMResponse>;
    
    /// Generate a streaming completion
    async fn generate_stream(
        &self,
        prompt: String,
        config: &LLMConfig,
    ) -> ServiceResult<Pin<Box<dyn Stream<Item = ServiceResult<String>> + Send>>>;
    
    /// List available models
    fn list_models(&self) -> Vec<String>;
    
    /// Get cost per 1M tokens (for optimization)
    fn cost_per_million_tokens(&self, model: &str) -> Option<f32>;
}

/// LLM configuration
#[derive(Debug, Clone)]
struct LLMConfig {
    model: String,
    temperature: f32,
    max_tokens: u32,
    top_p: f32,
    frequency_penalty: f32,
    presence_penalty: f32,
    stop_sequences: Vec<String>,
}

/// LLM response
#[derive(Debug, Clone)]
struct LLMResponse {
    text: String,
    usage: TokenUsage,
    model: String,
}

/// Token usage tracking
#[derive(Debug, Clone, Default)]
struct TokenUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// OpenRouter provider (multi-model API, cost-effective)
struct OpenRouterProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl OpenRouterProvider {
    fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://openrouter.ai/api/v1".to_string(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LLMProviderTrait for OpenRouterProvider {
    fn name(&self) -> &'static str {
        "openrouter"
    }
    
    #[instrument(skip(self, config), fields(provider = "openrouter", model = &config.model))]
    async fn generate_completion(
        &self,
        prompt: String,
        config: &LLMConfig,
    ) -> ServiceResult<LLMResponse> {
        // Build request body
        let request_body = serde_json::json!({
            "model": config.model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
            "top_p": config.top_p,
            "frequency_penalty": config.frequency_penalty,
            "presence_penalty": config.presence_penalty,
            "stop": config.stop_sequences
        });
        
        // Make API call
        let response = self.client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://meetily.community") // Required by OpenRouter
            .header("X-Title", "Meetily Community+")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError {
                provider: "OpenRouter".to_string(),
                message: format!("API request failed: {}", e),
            })?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError {
                provider: "OpenRouter".to_string(),
                message: format!("API returned {}: {}", status, error_text),
            });
        }
        
        // Parse response
        let result: serde_json::Value = response.json().await.map_err(|e| {
            AppError::ExternalServiceError {
                provider: "OpenRouter".to_string(),
                message: format!("Failed to parse response: {}", e),
            }
        })?;
        
        let text = result["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        
        let usage = TokenUsage {
            prompt_tokens: result["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: result["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: result["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
        };
        
        let model = result["model"].as_str().unwrap_or(&config.model).to_string();
        
        info!(
            "OpenRouter completion: {} tokens (prompt: {}, completion: {})",
            usage.total_tokens, usage.prompt_tokens, usage.completion_tokens
        );
        
        Ok(LLMResponse { text, usage, model })
    }
    
    async fn generate_stream(
        &self,
        prompt: String,
        config: &LLMConfig,
    ) -> ServiceResult<Pin<Box<dyn Stream<Item = ServiceResult<String>> + Send>>> {
        // OpenRouter supports streaming via SSE
        // TODO: Implement proper streaming with reqwest EventStream
        
        // For now, return placeholder error
        Err(AppError::ValidationError(
            "Streaming not yet implemented for OpenRouter provider".to_string()
        ))
    }
    
    fn list_models(&self) -> Vec<String> {
        vec![
            "anthropic/claude-3.5-sonnet".to_string(),
            "anthropic/claude-3-haiku".to_string(),
            "openai/gpt-4o".to_string(),
            "openai/gpt-4o-mini".to_string(),
            "google/gemini-flash-1.5".to_string(),
            "meta-llama/llama-3-70b-instruct".to_string(),
            "mistralai/mistral-large".to_string(),
            "qwen/qwen-2-72b-instruct".to_string(),
        ]
    }
    
    fn cost_per_million_tokens(&self, model: &str) -> Option<f32> {
        // Approximate costs from OpenRouter (as of 2024)
        Some(match model {
            m if m.contains("claude-3.5-sonnet") => 3.0,
            m if m.contains("claude-3-haiku") => 0.25,
            m if m.contains("gpt-4o") => 5.0,
            m if m.contains("gpt-4o-mini") => 0.15,
            m if m.contains("gemini-flash") => 0.075,
            m if m.contains("llama-3-70b") => 0.90,
            m if m.contains("mistral-large") => 4.0,
            m if m.contains("qwen-2-72b") => 0.90,
            _ => 1.0, // Default estimate
        })
    }
}

/// Ollama provider (local LLMs, free, private)
struct OllamaProvider {
    base_url: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LLMProviderTrait for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }
    
    #[instrument(skip(self, config), fields(provider = "ollama", model = &config.model))]
    async fn generate_completion(
        &self,
        prompt: String,
        config: &LLMConfig,
    ) -> ServiceResult<LLMResponse> {
        // Ollama API endpoint: /api/generate or /api/chat
        let request_body = serde_json::json!({
            "model": config.model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": config.temperature,
                "num_predict": config.max_tokens,
                "top_p": config.top_p,
            }
        });
        
        let response = self.client
            .post(&format!("{}/api/generate", self.base_url))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError {
                provider: "Ollama".to_string(),
                message: format!("Request failed: {}", e),
            })?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError {
                provider: "Ollama".to_string(),
                message: format!("Ollama returned {}: {}", status, error_text),
            });
        }
        
        let result: serde_json::Value = response.json().await.map_err(|e| {
            AppError::ExternalServiceError {
                provider: "Ollama".to_string(),
                message: format!("Failed to parse response: {}", e),
            }
        })?;
        
        let text = result["response"].as_str().unwrap_or("").to_string();
        
        // Ollama doesn't always return token counts
        let usage = TokenUsage {
            prompt_tokens: result["prompt_eval_count"].as_u64().unwrap_or(0) as u32,
            completion_tokens: result["eval_count"].as_u64().unwrap_or(0) as u32,
            total_tokens: result["prompt_eval_count"].as_u64().unwrap_or(0) as u32
                + result["eval_count"].as_u64().unwrap_or(0) as u32,
        };
        
        info!(
            "Ollama completion: {} tokens (model: {})",
            usage.total_tokens, config.model
        );
        
        Ok(LLMResponse {
            text,
            usage,
            model: config.model.clone(),
        })
    }
    
    async fn generate_stream(
        &self,
        prompt: String,
        config: &LLMConfig,
    ) -> ServiceResult<Pin<Box<dyn Stream<Item = ServiceResult<String>> + Send>>> {
        // Ollama supports streaming
        // TODO: Implement with reqwest EventStream
        Err(AppError::ValidationError(
            "Streaming not yet implemented for Ollama provider".to_string()
        ))
    }
    
    fn list_models(&self) -> Vec<String> {
        vec![
            "llama3.1:8b".to_string(),
            "llama3.1:70b".to_string(),
            "mistral:7b".to_string(),
            "mixtral:8x7b".to_string(),
            "gemma2:9b".to_string(),
            "qwen2.5:7b".to_string(),
            "qwen2.5:72b".to_string(),
            "codellama:7b".to_string(),
        ]
    }
    
    fn cost_per_million_tokens(&self, _model: &str) -> Option<f32> {
        // Ollama is free (local)
        Some(0.0)
    }
}

/// NVIDIA API provider (Nemotron models, fast)
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
impl LLMProviderTrait for NVIDIAProvider {
    fn name(&self) -> &'static str {
        "nvidia"
    }
    
    #[instrument(skip(self, config), fields(provider = "nvidia", model = &config.model))]
    async fn generate_completion(
        &self,
        prompt: String,
        config: &LLMConfig,
    ) -> ServiceResult<LLMResponse> {
        // NVIDIA API endpoint: /chat/completions (OpenAI-compatible)
        let request_body = serde_json::json!({
            "model": config.model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
            "top_p": config.top_p,
            "stream": false
        });
        
        let response = self.client
            .post(&format!("{}/chat/completions", self.base_url))
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
        
        let text = result["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        
        let usage = TokenUsage {
            prompt_tokens: result["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: result["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: result["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
        };
        
        info!(
            "NVIDIA completion: {} tokens (model: {})",
            usage.total_tokens, config.model
        );
        
        Ok(LLMResponse {
            text,
            usage,
            model: config.model.clone(),
        })
    }
    
    async fn generate_stream(
        &self,
        prompt: String,
        config: &LLMConfig,
    ) -> ServiceResult<Pin<Box<dyn Stream<Item = ServiceResult<String>> + Send>>> {
        // TODO: Implement streaming
        Err(AppError::ValidationError(
            "Streaming not yet implemented for NVIDIA provider".to_string()
        ))
    }
    
    fn list_models(&self) -> Vec<String> {
        vec![
            "nvidia/nemotron-4-340b-instruct".to_string(),
            "nvidia/nemotron-4-340b-reward".to_string(),
            "meta/llama-3.1-70b-instruct".to_string(),
            "meta/llama-3.1-405b-instruct".to_string(),
            "mistralai/mistral-large-2-instruct".to_string(),
            "google/gemma-2-27b-it".to_string(),
        ]
    }
    
    fn cost_per_million_tokens(&self, _model: &str) -> Option<f32> {
        // NVIDIA API free tier available
        Some(0.0) // Free tier
    }
}

/// Generic OpenAI-compatible provider (for Anthropic, custom endpoints, etc.)
struct OpenAICompatibleProvider {
    name: String,
    api_key: String,
    base_url: String,
    models: Vec<String>,
    client: reqwest::Client,
}

impl OpenAICompatibleProvider {
    fn new(
        name: String,
        api_key: String,
        base_url: String,
        models: Vec<String>,
    ) -> Self {
        Self {
            name,
            api_key,
            base_url,
            models,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LLMProviderTrait for OpenAICompatibleProvider {
    fn name(&self) -> &'static str {
        &self.name
    }
    
    async fn generate_completion(
        &self,
        prompt: String,
        config: &LLMConfig,
    ) -> ServiceResult<LLMResponse> {
        // Generic OpenAI-compatible endpoint
        let request_body = serde_json::json!({
            "model": config.model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
        });
        
        let response = self.client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError {
                provider: self.name.clone(),
                message: format!("API request failed: {}", e),
            })?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError {
                provider: self.name.clone(),
                message: format!("API returned {}: {}", status, error_text),
            });
        }
        
        let result: serde_json::Value = response.json().await.map_err(|e| {
            AppError::ExternalServiceError {
                provider: self.name.clone(),
                message: format!("Failed to parse response: {}", e),
            }
        })?;
        
        let text = result["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        
        let usage = TokenUsage {
            prompt_tokens: result["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: result["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: result["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
        };
        
        Ok(LLMResponse {
            text,
            usage,
            model: config.model.clone(),
        })
    }
    
    async fn generate_stream(
        &self,
        _prompt: String,
        _config: &LLMConfig,
    ) -> ServiceResult<Pin<Box<dyn Stream<Item = ServiceResult<String>> + Send>>> {
        Err(AppError::ValidationError(
            "Streaming not yet implemented".to_string()
        ))
    }
    
    fn list_models(&self) -> Vec<String> {
        self.models.clone()
    }
    
    fn cost_per_million_tokens(&self, _model: &str) -> Option<f32> {
        None // Unknown
    }
}

/// Main summary service implementation
pub struct SummaryServiceImpl {
    config: AppConfig,
    db_pool: Pool<Postgres>,
    providers: Arc<RwLock<Vec<Box<dyn LLMProviderTrait>>>>,
    prompt_templates: Arc<RwLock<HashMap<String, String>>>,
}

impl SummaryServiceImpl {
    /// Create a new summary service
    pub fn new(config: AppConfig, db_pool: Pool<Postgres>) -> Self {
        Self {
            config,
            db_pool,
            providers: Arc::new(RwLock::new(Vec::new())),
            prompt_templates: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Initialize providers based on configuration
    pub async fn initialize_providers(&self) -> ServiceResult<()> {
        let mut providers = self.providers.write().await;
        
        // Add OpenRouter provider if API key configured
        if let Some(api_key) = &self.config.openrouter_api_key {
            if !api_key.is_empty() {
                providers.push(Box::new(OpenRouterProvider::new(api_key.clone())));
                info!("Initialized OpenRouter provider");
            }
        }
        
        // Add Ollama provider (always available if running)
        let ollama_url = self.config.ollama_base_url.clone();
        providers.push(Box::new(OllamaProvider::new(ollama_url)));
        info!("Initialized Ollama provider at {}", ollama_url);
        
        // Add NVIDIA provider if API key configured
        if let Some(api_key) = &self.config.nvidia_api_key {
            if !api_key.is_empty() {
                providers.push(Box::new(NVIDIAProvider::new(
                    api_key.clone(),
                    self.config.nvidia_base_url.clone(),
                )));
                info!("Initialized NVIDIA provider");
            }
        }
        
        // Add custom OpenAI-compatible providers (e.g., Anthropic via proxy)
        // TODO: Read from config
        
        // Load default prompt templates
        self.load_default_prompt_templates().await;
        
        Ok(())
    }
    
    /// Load default prompt templates
    async fn load_default_prompt_templates(&self) {
        let mut templates = self.prompt_templates.write().await;
        
        // Executive summary template
        templates.insert(
            "executive".to_string(),
            r#"You are an expert meeting assistant. Generate a concise executive summary of the following meeting transcript.

Guidelines:
- Keep it under 200 words
- Focus on key points and outcomes
- Mention main topics discussed
- Note any critical decisions

Transcript:
{transcript}

Executive Summary:"#.to_string(),
        );
        
        // Technical summary template
        templates.insert(
            "technical".to_string(),
            r#"You are a technical analyst. Generate a detailed technical summary of the following meeting transcript.

Guidelines:
- Include technical details and specifications
- Note architecture decisions and trade-offs
- List technologies, tools, and frameworks discussed
- Document technical challenges and solutions

Transcript:
{transcript}

Technical Summary:"#.to_string(),
        );
        
        // Action items template
        templates.insert(
            "action_items".to_string(),
            r#"You are a project manager. Extract all action items from the following meeting transcript.

Guidelines:
- List each action item as a bullet point
- Include who is responsible (if mentioned)
- Include deadlines or timeframes (if mentioned)
- Format: "- [Owner] Action item [Deadline]"

Transcript:
{transcript}

Action Items:"#.to_string(),
        );
        
        // Decisions template
        templates.insert(
            "decisions".to_string(),
            r#"You are a business analyst. Extract all decisions made in the following meeting transcript.

Guidelines:
- List each decision as a bullet point
- Include context and rationale
- Note who made or approved the decision
- Format: "- Decision: [what] (Rationale: [why], Owner: [who])"

Transcript:
{transcript}

Decisions Made:"#.to_string(),
        );
        
        // Risks template
        templates.insert(
            "risks".to_string(),
            r#"You are a risk analyst. Identify all risks, concerns, and potential issues mentioned in the following meeting transcript.

Guidelines:
- List each risk as a bullet point
- Include severity (high/medium/low) if mentioned
- Note any mitigation strategies discussed
- Format: "- [Severity] Risk: [description] (Mitigation: [strategy])"

Transcript:
{transcript}

Risks and Concerns:"#.to_string(),
        );
        
        // Follow-up tasks template
        templates.insert(
            "follow_up".to_string(),
            r#"You are a coordinator. Extract all follow-up tasks and next steps from the following meeting transcript.

Guidelines:
- List tasks that need to be done after this meeting
- Include owners and deadlines
- Note dependencies between tasks
- Format: "- [Owner] Task [Deadline] (Dependencies: [what])"

Transcript:
{transcript}

Follow-up Tasks:"#.to_string(),
        );
        
        info!("Loaded {} default prompt templates", templates.len());
    }
    
    /// Get provider by name
    async fn get_provider(&self, name: &str) -> ServiceResult<Arc<dyn LLMProviderTrait>> {
        let providers = self.providers.read().await;
        
        for provider in providers.iter() {
            if provider.name() == name {
                // TODO: Fix provider cloning
                return Err(AppError::ServiceError("Provider cloning not implemented".to_string()));
            }
        }
        
        Err(AppError::NotFound(format!("LLM provider '{}' not found", name)))
    }
    
    /// Get best provider (cheapest available)
    async fn get_best_provider(&self, model_preference: Option<&str>) -> ServiceResult<Arc<dyn LLMProviderTrait>> {
        let providers = self.providers.read().await;
        
        if providers.is_empty() {
            return Err(AppError::ServiceError("No LLM providers initialized".to_string()));
        }
        
        // If model preference specified, find provider that supports it
        if let Some(model) = model_preference {
            for provider in providers.iter() {
                if provider.list_models().iter().any(|m| m.contains(model)) {
                    // TODO: Fix provider cloning
                    return Err(AppError::ServiceError("Provider cloning not implemented".to_string()));
                }
            }
        }
        
        // Otherwise, return cheapest provider
        // Priority: Ollama (free) > NVIDIA (free tier) > OpenRouter (cheap models)
        for provider in providers.iter() {
            if provider.name() == "ollama" {
                return Err(AppError::ServiceError("Provider cloning not implemented".to_string()));
            }
        }
        
        // Return first available
        Err(AppError::ServiceError("Provider cloning not implemented".to_string()))
    }
    
    /// Render prompt template with transcript
    fn render_prompt(&self, template_name: &str, transcript: &str) -> ServiceResult<String> {
        let templates = self.prompt_templates.blocking_read();
        
        let template = templates.get(template_name)
            .ok_or_else(|| AppError::NotFound(format!("Prompt template '{}' not found", template_name)))?;
        
        // Replace {transcript} placeholder
        Ok(template.replace("{transcript}", transcript))
    }
}

#[async_trait]
impl SummaryService for SummaryServiceImpl {
    #[instrument(skip(self), fields(meeting_id = %meeting_id, summary_type = ?summary_type))]
    async fn generate_summary(
        &self,
        meeting_id: Uuid,
        summary_type: SummaryType,
        custom_prompt: Option<String>,
    ) -> ServiceResult<Summary> {
        // Initialize providers if not already done
        {
            let providers = self.providers.read().await;
            if providers.is_empty() {
                drop(providers);
                self.initialize_providers().await?;
            }
        }
        
        // Load transcript segments for this meeting
        let transcript_segments = self.load_transcript_segments(meeting_id).await?;
        
        if transcript_segments.is_empty() {
            return Err(AppError::NotFound(format!(
                "No transcript found for meeting {}",
                meeting_id
            )));
        }
        
        // Combine transcript segments into single text
        let full_transcript = transcript_segments
            .iter()
            .map(|s| format!("[{}-{}] {}: {}", 
                format_timestamp(s.start_time_secs),
                format_timestamp(s.end_time_secs),
                s.speaker_id.as_deref().unwrap_or("Unknown"),
                s.text
            ))
            .collect::<Vec<_>>()
            .join("\n");
        
        // Build prompt
        let prompt = if let Some(custom) = custom_prompt {
            custom.replace("{transcript}", &full_transcript)
        } else {
            let template_name = match summary_type {
                SummaryType::Executive => "executive",
                SummaryType::Technical => "technical",
                SummaryType::ActionItems => "action_items",
                SummaryType::Decisions => "decisions",
                SummaryType::Risks => "risks",
                SummaryType::FollowUp => "follow_up",
                SummaryType::Custom => "executive", // Default
            };
            
            self.render_prompt(template_name, &full_transcript)?
        };
        
        // Get best provider
        let provider = self.get_best_provider(None).await?;
        
        // Default LLM config
        let llm_config = LLMConfig {
            model: "llama3.1:8b".to_string(), // Default to Ollama
            temperature: 0.3,
            max_tokens: 1024,
            top_p: 0.9,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            stop_sequences: vec![],
        };
        
        // Generate summary
        info!("Generating {:?} summary for meeting {}", summary_type, meeting_id);
        let response = provider.generate_completion(prompt, &llm_config).await?;
        
        // Create summary record
        let summary = Summary {
            id: Uuid::new_v4(),
            meeting_id,
            summary_type,
            content: response.text,
            model_used: response.model,
            provider: provider.name().to_string(),
            token_usage: Some(SummaryMetadata {
                prompt_tokens: response.usage.prompt_tokens,
                completion_tokens: response.usage.completion_tokens,
                total_tokens: response.usage.total_tokens,
                cost_usd: None, // TODO: Calculate from cost_per_million_tokens
            }),
            created_at: Utc::now(),
        };
        
        // Save to database
        self.save_summary(&summary).await?;
        
        info!(
            "Generated {:?} summary for meeting {} ({} tokens, provider: {})",
            summary_type, meeting_id, response.usage.total_tokens, provider.name()
        );
        
        Ok(summary)
    }
    
    async fn generate_all_summaries(
        &self,
        meeting_id: Uuid,
        custom_prompts: Option<HashMap<SummaryType, String>>,
    ) -> ServiceResult<Vec<Summary>> {
        let summary_types = vec![
            SummaryType::Executive,
            SummaryType::Technical,
            SummaryType::ActionItems,
            SummaryType::Decisions,
            SummaryType::Risks,
            SummaryType::FollowUp,
        ];
        
        let mut summaries = Vec::new();
        
        for summary_type in summary_types {
            let custom_prompt = custom_prompts.as_ref()
                .and_then(|prompts| prompts.get(&summary_type).cloned());
            
            match self.generate_summary(meeting_id, summary_type.clone(), custom_prompt).await {
                Ok(summary) => summaries.push(summary),
                Err(e) => {
                    warn!("Failed to generate {:?} summary: {}", summary_type, e);
                    // Continue with other summary types
                }
            }
        }
        
        Ok(summaries)
    }
    
    async fn get_summary(&self, summary_id: Uuid) -> ServiceResult<Summary> {
        let record = sqlx::query_as!(
            SummaryRow,
            r#"
            SELECT 
                id, meeting_id, summary_type, content, model_used,
                provider, metadata_json, created_at
            FROM summaries
            WHERE id = $1
            "#,
            summary_id
        )
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        match record {
            Some(row) => Ok(row.into()),
            None => Err(AppError::NotFound(format!("Summary {} not found", summary_id))),
        }
    }
    
    async fn get_meeting_summaries(&self, meeting_id: Uuid) -> ServiceResult<Vec<Summary>> {
        let records = sqlx::query_as!(
            SummaryRow,
            r#"
            SELECT 
                id, meeting_id, summary_type, content, model_used,
                provider, metadata_json, created_at
            FROM summaries
            WHERE meeting_id = $1
            ORDER BY created_at ASC
            "#,
            meeting_id
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(records.into_iter().map(|r| r.into()).collect())
    }
    
    async fn delete_summary(&self, summary_id: Uuid) -> ServiceResult<()> {
        sqlx::query!("DELETE FROM summaries WHERE id = $1", summary_id)
            .execute(&self.db_pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        info!("Deleted summary {}", summary_id);
        Ok(())
    }
    
    async fn save_custom_prompt_template(
        &self,
        name: String,
        template: String,
    ) -> ServiceResult<()> {
        let mut templates = self.prompt_templates.write().await;
        templates.insert(name, template);
        Ok(())
    }
    
    async fn list_available_models(&self) -> ServiceResult<Vec<String>> {
        let providers = self.providers.read().await;
        let mut models = Vec::new();
        
        for provider in providers.iter() {
            models.extend(provider.list_models());
        }
        
        Ok(models)
    }
}

// Database helper methods
impl SummaryServiceImpl {
    async fn load_transcript_segments(&self, meeting_id: Uuid) 
        -> ServiceResult<Vec<crate::services::transcription::TranscriptionSegment> 
    {
        // TODO: Implement
        Ok(vec![])
    }
    
    async fn save_summary(&self, summary: &Summary) -> ServiceResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO summaries (
                id, meeting_id, summary_type, content, model_used,
                provider, metadata_json
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            summary.id,
            summary.meeting_id,
            summary.summary_type.to_string(),
            &summary.content,
            &summary.model_used,
            &summary.provider,
            summary.token_usage.as_ref()
                .map(|m| serde_json::to_value(m).unwrap())
                .unwrap_or(serde_json::Value::Null)
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
}

// Helper function
fn format_timestamp(secs: f32) -> String {
    let minutes = (secs / 60.0) as u32;
    let seconds = (secs % 60.0) as u32;
    format!("{:02}:{:02}", minutes, seconds)
}

// Database row mapping
#[derive(sqlx::FromRow)]
struct SummaryRow {
    id: Uuid,
    meeting_id: Uuid,
    summary_type: String,
    content: String,
    model_used: String,
    provider: String,
    metadata_json: serde_json::Value,
    created_at: DateTime<Utc>,
}

impl From<SummaryRow> for Summary {
    fn from(row: SummaryRow) -> Self {
        Self {
            id: row.id,
            meeting_id: row.meeting_id,
            summary_type: match row.summary_type.as_str() {
                "executive" => SummaryType::Executive,
                "technical" => SummaryType::Technical,
                "action_items" => SummaryType::ActionItems,
                "decisions" => SummaryType::Decisions,
                "risks" => SummaryType::Risks,
                "follow_up" => SummaryType::FollowUp,
                "custom" => SummaryType::Custom,
                _ => SummaryType::Executive,
            },
            content: row.content,
            model_used: row.model_used,
            provider: row.provider,
            token_usage: serde_json::from_value(row.metadata_json).ok(),
            created_at: row.created_at,
        }
    }
}

// Database schema (for migrations)
// CREATE TABLE IF NOT EXISTS summaries (
//     id UUID PRIMARY KEY,
//     meeting_id UUID NOT NULL,
//     summary_type TEXT NOT NULL,
//     content TEXT NOT NULL,
//     model_used TEXT NOT NULL,
//     provider TEXT NOT NULL,
//     metadata_json JSONB,
//     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
// );
//
// CREATE INDEX IF NOT EXISTS idx_summaries_meeting_id ON summaries(meeting_id);
// CREATE INDEX IF NOT EXISTS idx_summaries_type ON summaries(summary_type);
// CREATE INDEX IF NOT EXISTS idx_summaries_created_at ON summaries(created_at DESC);