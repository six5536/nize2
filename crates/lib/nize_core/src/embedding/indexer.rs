// @zen-component: EMB-ToolIndexer
//
//! Tool embedding indexer — generates and stores embeddings for MCP server tools.
//!
//! After tools are saved via [`crate::mcp::queries::replace_server_tools`],
//! call [`embed_server_tools`] to generate embeddings for semantic discovery.

use std::sync::Arc;

use reqwest::Client;
use sqlx::PgPool;
use tokio::sync::RwLock;

use crate::config::cache::ConfigCache;
use crate::mcp;
use crate::uuid::uuidv7;

use super::EmbeddingError;
use super::config::EmbeddingConfig;
use super::models;
use super::provider;

// @zen-impl: MCP-7_AC-2
/// Build embedding text by concatenating server context with tool description.
///
/// Matches the reference implementation's `buildEmbeddingText()`.
pub fn build_embedding_text(
    server_name: &str,
    server_description: &str,
    tool_description: &str,
) -> String {
    let mut parts = vec![format!("Server: {server_name}")];
    if !server_description.is_empty() {
        parts.push(server_description.to_string());
    }
    parts.push(tool_description.to_string());
    parts.join("\n\n")
}

/// Generate and store embeddings for all tools belonging to an MCP server.
///
/// This function:
/// 1. Resolves the embedding config (provider/model/keys)
/// 2. Fetches server info for embedding context
/// 3. Fetches current tool rows (with their UUIDs)
/// 4. For each tool, generates an embedding and upserts into the tool embedding table
///
/// Returns the number of tools successfully embedded.
///
/// Errors are returned (not swallowed) — callers should log and continue.
pub async fn embed_server_tools(
    pool: &PgPool,
    config_cache: &Arc<RwLock<ConfigCache>>,
    server_id: &str,
    encryption_key: &str,
) -> Result<usize, EmbeddingError> {
    // Resolve embedding config
    let config = EmbeddingConfig::resolve(pool, config_cache, encryption_key).await?;
    let model_config = models::get_active_model(pool, &config).await?;

    // Fetch server info
    let server = mcp::queries::get_server(pool, server_id)
        .await
        .map_err(|e| EmbeddingError::Provider(format!("Failed to fetch server: {e}")))?
        .ok_or_else(|| EmbeddingError::Provider(format!("Server {server_id} not found")))?;

    // Fetch current tool rows
    let tools = mcp::queries::list_server_tools(pool, server_id)
        .await
        .map_err(|e| EmbeddingError::Provider(format!("Failed to fetch tools: {e}")))?;

    if tools.is_empty() {
        return Ok(0);
    }

    let client = Client::new();
    let mut count = 0;

    for tool in &tools {
        let embedding_text =
            build_embedding_text(&server.name, &server.description, &tool.description);

        // Generate embedding
        let texts = vec![embedding_text];
        let results = provider::embed_with_model(&client, &config, &texts, &model_config).await?;

        let embedding = results
            .into_iter()
            .next()
            .map(|r| r.embedding)
            .ok_or_else(|| EmbeddingError::Provider("No embedding result returned".to_string()))?;

        // Format vector as SQL literal: '[0.1,0.2,...]'
        let embedding_sql: String = format!(
            "[{}]",
            embedding
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        // Upsert into dynamic tool embedding table
        let query = format!(
            r#"INSERT INTO "{}" (id, tool_id, server_id, domain, embedding)
               VALUES ($1, $2, $3, $4, $5::vector)
               ON CONFLICT (tool_id) DO UPDATE SET
                 embedding = EXCLUDED.embedding,
                 domain = EXCLUDED.domain"#,
            model_config.tool_table_name
        );

        sqlx::query(&query)
            .bind(uuidv7())
            .bind(tool.id)
            .bind(server.id)
            .bind(&server.domain)
            .bind(&embedding_sql)
            .execute(pool)
            .await
            .map_err(EmbeddingError::Db)?;

        count += 1;
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_embedding_text_full() {
        let text = build_embedding_text("MyServer", "A useful server", "Search the web");
        assert_eq!(
            text,
            "Server: MyServer\n\nA useful server\n\nSearch the web"
        );
    }

    #[test]
    fn build_embedding_text_empty_description() {
        let text = build_embedding_text("MyServer", "", "Search the web");
        assert_eq!(text, "Server: MyServer\n\nSearch the web");
    }
}
