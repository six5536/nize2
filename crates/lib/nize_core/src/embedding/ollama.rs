// @awa-component: EMB-OllamaProvider
//
//! Ollama embedding provider.
//!
//! Calls the Ollama API (`/api/embeddings`) to generate embeddings.
//! Texts are embedded sequentially (one at a time) as Ollama only accepts
//! a single prompt per request.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::config::EmbeddingConfig;
use super::models::EmbeddingModelConfig;
use super::{EmbeddingError, EmbeddingResult};

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}

#[derive(Deserialize)]
struct OllamaResponse {
    embedding: Option<Vec<f64>>,
}

/// Embed a single text via Ollama.
async fn embed_one(
    client: &Client,
    config: &EmbeddingConfig,
    text: &str,
    model_config: &EmbeddingModelConfig,
) -> Result<Vec<f32>, EmbeddingError> {
    let url = format!("{}/api/embeddings", config.ollama_base_url);

    let resp = client
        .post(&url)
        .json(&OllamaRequest {
            model: &model_config.model,
            prompt: text,
        })
        .send()
        .await
        .map_err(|e| EmbeddingError::Provider(format!("Ollama request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        return Err(EmbeddingError::Provider(format!(
            "Ollama embeddings failed: {status} {body}"
        )));
    }

    let data: OllamaResponse = resp
        .json()
        .await
        .map_err(|e| EmbeddingError::Provider(format!("Ollama response parse error: {e}")))?;

    let embedding: Vec<f32> = data
        .embedding
        .unwrap_or_default()
        .into_iter()
        .map(|v| v as f32)
        .collect();

    if embedding.len() != model_config.dimensions as usize {
        return Err(EmbeddingError::DimensionMismatch {
            expected: model_config.dimensions,
            actual: embedding.len() as i32,
        });
    }

    Ok(embedding)
}

/// Embed a batch of texts sequentially via Ollama.
pub async fn embed_batch(
    client: &Client,
    config: &EmbeddingConfig,
    texts: &[String],
    model_config: &EmbeddingModelConfig,
) -> Result<Vec<EmbeddingResult>, EmbeddingError> {
    let mut results = Vec::with_capacity(texts.len());
    for text in texts {
        let embedding = embed_one(client, config, text, model_config).await?;
        results.push(EmbeddingResult {
            text: text.clone(),
            embedding,
            model: model_config.model.clone(),
        });
    }
    Ok(results)
}
