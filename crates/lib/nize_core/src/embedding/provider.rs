// @awa-component: EMB-ProviderDispatch
//
//! Provider dispatch — routes embedding requests to the correct provider.

use reqwest::Client;

use super::config::EmbeddingConfig;
use super::models::EmbeddingModelConfig;
use super::{EmbeddingError, EmbeddingResult, local, ollama, openai};

/// Generate embeddings for a batch of texts using a specific model.
///
/// Dispatches based on `model_config.provider`:
/// - `"openai"` → OpenAI API with retry
/// - `"ollama"` → Ollama local API
/// - `"local"` → deterministic FNV hash
pub async fn embed_with_model(
    client: &Client,
    config: &EmbeddingConfig,
    texts: &[String],
    model_config: &EmbeddingModelConfig,
) -> Result<Vec<EmbeddingResult>, EmbeddingError> {
    match model_config.provider.as_str() {
        "local" => Ok(local::embed_batch(
            texts,
            model_config.dimensions,
            &model_config.model,
        )),
        "ollama" => ollama::embed_batch(client, config, texts, model_config).await,
        "openai" => openai::embed_batch(client, config, texts, model_config).await,
        other => Err(EmbeddingError::UnsupportedProvider(other.to_string())),
    }
}
