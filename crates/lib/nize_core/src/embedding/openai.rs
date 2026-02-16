// @zen-component: EMB-OpenAIProvider
//
//! OpenAI embedding provider.
//!
//! Calls the OpenAI embeddings API (`/v1/embeddings`) with retry logic
//! (max 3 attempts, exponential backoff).

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::{Duration, sleep};

use super::config::EmbeddingConfig;
use super::models::EmbeddingModelConfig;
use super::{EmbeddingError, EmbeddingResult};

const MAX_RETRY_ATTEMPTS: u32 = 3;
const OPENAI_API_URL: &str = "https://api.openai.com/v1/embeddings";

#[derive(Serialize)]
struct OpenAIRequest<'a> {
    model: &'a str,
    input: &'a str,
    dimensions: i32,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    data: Vec<OpenAIEmbedding>,
}

#[derive(Deserialize)]
struct OpenAIEmbedding {
    embedding: Vec<f64>,
}

/// Embed a single text via OpenAI with retry.
async fn embed_one(
    client: &Client,
    config: &EmbeddingConfig,
    text: &str,
    model_config: &EmbeddingModelConfig,
) -> Result<Vec<f32>, EmbeddingError> {
    let api_key = config
        .openai_api_key
        .as_deref()
        .ok_or_else(|| EmbeddingError::Config("OPENAI_API_KEY is required for openai provider".to_string()))?;

    let mut last_error = None;

    for attempt in 0..MAX_RETRY_ATTEMPTS {
        let result = client
            .post(OPENAI_API_URL)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&OpenAIRequest {
                model: &model_config.model,
                input: text,
                dimensions: model_config.dimensions,
            })
            .send()
            .await;

        match result {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp
                        .text()
                        .await
                        .unwrap_or_else(|_| "<no body>".to_string());
                    last_error = Some(EmbeddingError::Provider(format!(
                        "OpenAI embeddings failed: {status} {body}"
                    )));
                } else {
                    let data: OpenAIResponse = resp.json().await.map_err(|e| {
                        EmbeddingError::Provider(format!("OpenAI response parse error: {e}"))
                    })?;

                    let embedding: Vec<f32> = data
                        .data
                        .into_iter()
                        .next()
                        .ok_or_else(|| {
                            EmbeddingError::Provider(
                                "OpenAI returned empty data array".to_string(),
                            )
                        })?
                        .embedding
                        .into_iter()
                        .map(|v| v as f32)
                        .collect();

                    return Ok(embedding);
                }
            }
            Err(e) => {
                last_error = Some(EmbeddingError::Provider(format!(
                    "OpenAI request failed: {e}"
                )));
            }
        }

        // Exponential backoff before retry
        if attempt + 1 < MAX_RETRY_ATTEMPTS {
            let backoff = Duration::from_secs(2u64.pow(attempt + 1));
            sleep(backoff).await;
        }
    }

    Err(last_error.unwrap_or_else(|| {
        EmbeddingError::Provider(format!(
            "Failed to embed after {MAX_RETRY_ATTEMPTS} attempts"
        ))
    }))
}

/// Embed a batch of texts via OpenAI (one at a time with retry).
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
