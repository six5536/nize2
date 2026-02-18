// @zen-component: MCP-DiscoveryService
//
//! Semantic tool discovery — pgvector similarity search for MCP tools.
//!
//! Accepts a query string, embeds it via the embedding subsystem, and
//! searches the tool embedding table using cosine similarity. Results
//! are filtered by user-enabled servers (via `user_mcp_preferences`).

use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::cache::ConfigCache;
use crate::embedding;
use crate::embedding::config::EmbeddingConfig;
use crate::embedding::models;

use super::McpError;

/// Parameters for a tool discovery search.
#[derive(Debug, Clone)]
pub struct DiscoveryQuery {
    pub query: String,
    pub domain: Option<String>,
    pub user_id: String,
    pub top_k: Option<i64>,
    pub min_similarity: Option<f64>,
}

/// A row from a tool discovery search result.
#[derive(Debug, Clone)]
pub struct DiscoveredToolRow {
    pub tool_id: Uuid,
    pub tool_name: String,
    pub tool_description: String,
    pub domain: String,
    pub server_id: Uuid,
    pub server_name: String,
    pub server_description: String,
    pub similarity: f64,
}

/// Discover tools by semantic similarity search.
///
/// Embeds the query via the active embedding model, then runs a cosine
/// similarity search against the tool embedding table. Results are filtered
/// by servers the user has enabled (or that are globally visible with no
/// explicit opt-out).
pub async fn discover_tools(
    pool: &PgPool,
    config_cache: &Arc<RwLock<ConfigCache>>,
    query: &DiscoveryQuery,
    encryption_key: &str,
) -> Result<Vec<DiscoveredToolRow>, McpError> {
    // Resolve embedding config
    let config = EmbeddingConfig::resolve(pool, config_cache, encryption_key)
        .await
        .map_err(|e| McpError::ConnectionFailed(format!("Embedding config error: {e}")))?;

    // Get active model to know which table to search
    let model_config = models::get_active_model(pool, &config)
        .await
        .map_err(|e| McpError::ConnectionFailed(format!("Embedding model error: {e}")))?;

    // Embed the query
    let query_embedding = embedding::embed_single(pool, &config, &query.query)
        .await
        .map_err(|e| McpError::ConnectionFailed(format!("Embedding error: {e}")))?;

    // Format vector as SQL literal: '[0.1,0.2,...]'
    let embedding_sql = format!(
        "[{}]",
        query_embedding
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );

    let top_k = query.top_k.unwrap_or(10);
    let min_similarity = query.min_similarity.unwrap_or(0.5);

    // Build the similarity search query with user preference filter.
    // A user sees tools from servers where:
    //   1. The server is enabled AND visible, AND
    //   2. The user hasn't explicitly disabled it,  OR
    //   3. The user has explicitly enabled it.
    let sql = if query.domain.is_some() {
        format!(
            r#"SELECT t.id AS tool_id,
                      t.name AS tool_name,
                      t.description AS tool_description,
                      te.domain,
                      s.id AS server_id,
                      s.name AS server_name,
                      s.description AS server_description,
                      1 - (te.embedding <=> $1::vector) AS similarity
               FROM "{tool_table}" te
               JOIN mcp_server_tools t ON t.id = te.tool_id
               JOIN mcp_servers s ON s.id = te.server_id
               WHERE s.enabled = true
                 AND te.domain = $4
                 AND (
                   (s.visibility = 'visible' AND NOT EXISTS (
                     SELECT 1 FROM user_mcp_preferences p
                     WHERE p.user_id = $5::uuid AND p.server_id = s.id AND p.enabled = false
                   ))
                   OR EXISTS (
                     SELECT 1 FROM user_mcp_preferences p
                     WHERE p.user_id = $5::uuid AND p.server_id = s.id AND p.enabled = true
                   )
                 )
                 AND 1 - (te.embedding <=> $1::vector) >= $3
               ORDER BY te.embedding <=> $1::vector
               LIMIT $2"#,
            tool_table = model_config.tool_table_name
        )
    } else {
        format!(
            r#"SELECT t.id AS tool_id,
                      t.name AS tool_name,
                      t.description AS tool_description,
                      te.domain,
                      s.id AS server_id,
                      s.name AS server_name,
                      s.description AS server_description,
                      1 - (te.embedding <=> $1::vector) AS similarity
               FROM "{tool_table}" te
               JOIN mcp_server_tools t ON t.id = te.tool_id
               JOIN mcp_servers s ON s.id = te.server_id
               WHERE s.enabled = true
                 AND (
                   (s.visibility = 'visible' AND NOT EXISTS (
                     SELECT 1 FROM user_mcp_preferences p
                     WHERE p.user_id = $4::uuid AND p.server_id = s.id AND p.enabled = false
                   ))
                   OR EXISTS (
                     SELECT 1 FROM user_mcp_preferences p
                     WHERE p.user_id = $4::uuid AND p.server_id = s.id AND p.enabled = true
                   )
                 )
                 AND 1 - (te.embedding <=> $1::vector) >= $3
               ORDER BY te.embedding <=> $1::vector
               LIMIT $2"#,
            tool_table = model_config.tool_table_name
        )
    };

    let rows = if query.domain.is_some() {
        sqlx::query_as::<_, (Uuid, String, String, String, Uuid, String, String, f64)>(&sql)
            .bind(&embedding_sql)
            .bind(top_k)
            .bind(min_similarity)
            .bind(query.domain.as_deref().unwrap_or(""))
            .bind(&query.user_id)
            .fetch_all(pool)
            .await
            .map_err(McpError::DbError)?
    } else {
        sqlx::query_as::<_, (Uuid, String, String, String, Uuid, String, String, f64)>(&sql)
            .bind(&embedding_sql)
            .bind(top_k)
            .bind(min_similarity)
            .bind(&query.user_id)
            .fetch_all(pool)
            .await
            .map_err(McpError::DbError)?
    };

    Ok(rows
        .into_iter()
        .map(
            |(
                tool_id,
                tool_name,
                tool_description,
                domain,
                server_id,
                server_name,
                server_description,
                similarity,
            )| {
                DiscoveredToolRow {
                    tool_id,
                    tool_name,
                    tool_description,
                    domain,
                    server_id,
                    server_name,
                    server_description,
                    similarity,
                }
            },
        )
        .collect())
}
