// @awa-component: EMB-EmbeddingAPI
//
//! Embedding module — text embedding generation for MCP tool discovery.
//!
//! Supports multiple providers (OpenAI, Ollama, local/deterministic) and
//! resolves configuration from the admin config system with env var fallback.
//!
//! # Public API
//!
//! - [`embed`] — embed multiple texts using all models for the active provider
//! - [`embed_single`] — embed a single text using the active model
//! - [`models::get_model_configs`] — get registered models for a provider
//! - [`models::get_active_model`] — get the active model config
//! - [`config::EmbeddingConfig`] — resolved embedding configuration
//!
//! # Providers
//!
//! - `"openai"` — OpenAI API (`text-embedding-3-small`)
//! - `"ollama"` — Ollama local API (`nomic-embed-text`)
//! - `"local"` — Deterministic FNV-1a hash (offline, no external deps)

pub mod config;
pub mod indexer;
pub mod local;
pub mod models;
pub mod ollama;
pub mod openai;
pub mod provider;

use reqwest::Client;
use sqlx::PgPool;
use thiserror::Error;

use config::EmbeddingConfig;

/// Errors that can occur during embedding operations.
#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("No models registered for provider: {0}")]
    NoModels(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Unsupported provider: {0}")]
    UnsupportedProvider(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: i32, actual: i32 },
}

/// Result of embedding a single text.
#[derive(Debug, Clone)]
pub struct EmbeddingResult {
    pub text: String,
    pub embedding: Vec<f32>,
    pub model: String,
}

/// Embed multiple texts using ALL models for the active provider.
///
/// Returns one `EmbeddingResult` per text per model.
pub async fn embed(
    pool: &PgPool,
    config: &EmbeddingConfig,
    texts: &[String],
) -> Result<Vec<EmbeddingResult>, EmbeddingError> {
    let model_configs = models::get_model_configs(pool, &config.provider).await?;
    let client = Client::new();
    let mut all_results = Vec::new();

    for model_config in &model_configs {
        let results = provider::embed_with_model(&client, config, texts, model_config).await?;
        all_results.extend(results);
    }

    Ok(all_results)
}

/// Embed a single text using the active model only.
///
/// Returns the embedding vector.
pub async fn embed_single(
    pool: &PgPool,
    config: &EmbeddingConfig,
    text: &str,
) -> Result<Vec<f32>, EmbeddingError> {
    let model_config = models::get_active_model(pool, config).await?;
    let client = Client::new();
    let texts = vec![text.to_string()];
    let results = provider::embed_with_model(&client, config, &texts, &model_config).await?;

    results
        .into_iter()
        .next()
        .map(|r| r.embedding)
        .ok_or_else(|| EmbeddingError::Provider("No embedding result returned".to_string()))
}
