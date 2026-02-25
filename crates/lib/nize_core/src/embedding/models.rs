// @awa-component: EMB-EmbeddingModels
//
//! Database queries for the embedding model registry.

use sqlx::PgPool;

use super::EmbeddingError;
use super::config::EmbeddingConfig;

/// A registered embedding model from the `embedding_models` table.
#[derive(Debug, Clone)]
pub struct EmbeddingModelConfig {
    pub provider: String,
    pub model: String,
    pub dimensions: i32,
    pub table_name: String,
    pub tool_table_name: String,
}

/// Get all registered models for a given provider.
pub async fn get_model_configs(
    pool: &PgPool,
    provider: &str,
) -> Result<Vec<EmbeddingModelConfig>, EmbeddingError> {
    let rows = sqlx::query_as::<_, (String, String, i32, String, String)>(
        "SELECT provider, name, dimensions, table_name, tool_table_name \
         FROM embedding_models WHERE provider = $1 ORDER BY name",
    )
    .bind(provider)
    .fetch_all(pool)
    .await
    .map_err(EmbeddingError::Db)?;

    if rows.is_empty() {
        return Err(EmbeddingError::NoModels(provider.to_string()));
    }

    Ok(rows
        .into_iter()
        .map(
            |(provider, name, dimensions, table_name, tool_table_name)| EmbeddingModelConfig {
                provider,
                model: name,
                dimensions,
                table_name,
                tool_table_name,
            },
        )
        .collect())
}

/// Get the active model config matching the active model name in config.
pub async fn get_active_model(
    pool: &PgPool,
    config: &EmbeddingConfig,
) -> Result<EmbeddingModelConfig, EmbeddingError> {
    let configs = get_model_configs(pool, &config.provider).await?;
    configs
        .into_iter()
        .find(|c| c.model == config.active_model)
        .ok_or_else(|| EmbeddingError::ModelNotFound(config.active_model.clone()))
}
