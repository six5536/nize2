// @awa-component: PLAN-023-EmbeddingsAdmin
//
//! Admin embedding management endpoints.

use axum::Json;
use axum::extract::State;
use base64::{Engine, engine::general_purpose};
use reqwest::Client;
use serde::Deserialize;

use crate::AppState;
use crate::error::{AppError, AppResult};

fn encode_page_token(offset: i64) -> String {
    general_purpose::STANDARD_NO_PAD.encode(offset.to_be_bytes())
}

fn decode_page_token(token: &str) -> Option<i64> {
    let bytes = general_purpose::STANDARD_NO_PAD.decode(token).ok()?;
    if bytes.len() == 8 {
        let mut arr = [0u8; 8];
        arr.copy_from_slice(&bytes);
        Some(i64::from_be_bytes(arr))
    } else {
        None
    }
}

/// `GET /admin/embeddings/models` — list registered embedding models.
pub async fn list_models_handler(
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let rows = sqlx::query_as::<_, (String, String, i32, String, String)>(
        "SELECT provider, name, dimensions, table_name, tool_table_name \
         FROM embedding_models ORDER BY provider, name",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to list embedding models: {e}")))?;

    // Resolve active model name from config
    let active_model_config = nize_core::embedding::config::EmbeddingConfig::resolve(
        &state.pool,
        &state.config_cache,
        &state.config.mcp_encryption_key,
    )
    .await;
    let active_model = active_model_config
        .as_ref()
        .map(|c| c.active_model.as_str())
        .unwrap_or("");
    let active_provider = active_model_config
        .as_ref()
        .map(|c| c.provider.as_str())
        .unwrap_or("");

    let models: Vec<serde_json::Value> = rows
        .into_iter()
        .map(
            |(provider, name, dimensions, table_name, tool_table_name)| {
                let is_active = provider == active_provider && name == active_model;
                serde_json::json!({
                    "provider": provider,
                    "name": name,
                    "dimensions": dimensions,
                    "tableName": table_name,
                    "toolTableName": tool_table_name,
                    "isActive": is_active,
                })
            },
        )
        .collect();

    Ok(Json(serde_json::json!({ "models": models })))
}

/// Search request body.
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(rename = "pageSize")]
    pub page_size: Option<i64>,
    #[serde(rename = "nextToken")]
    pub next_token: Option<String>,
}

/// `POST /admin/embeddings/search` — embed a query and return ranked tool matches.
pub async fn search_handler(
    State(state): State<AppState>,
    Json(body): Json<SearchRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if body.query.trim().is_empty() {
        return Err(AppError::Validation("query is required".into()));
    }

    let page_size = body.page_size.unwrap_or(20).clamp(1, 200);
    let offset = body
        .next_token
        .as_deref()
        .and_then(decode_page_token)
        .unwrap_or(0);

    // Resolve embedding config
    let config = nize_core::embedding::config::EmbeddingConfig::resolve(
        &state.pool,
        &state.config_cache,
        &state.config.mcp_encryption_key,
    )
    .await
    .map_err(|e| AppError::Internal(format!("Embedding config error: {e}")))?;

    // Get active model
    let model_config = nize_core::embedding::models::get_active_model(&state.pool, &config)
        .await
        .map_err(|e| AppError::Internal(format!("Embedding model error: {e}")))?;

    // Embed the query
    let client = Client::new();
    let texts = vec![body.query.clone()];
    let results =
        nize_core::embedding::provider::embed_with_model(&client, &config, &texts, &model_config)
            .await
            .map_err(|e| AppError::Internal(format!("Embedding error: {e}")))?;

    let embedding = results
        .into_iter()
        .next()
        .map(|r| r.embedding)
        .ok_or_else(|| AppError::Internal("No embedding result returned".into()))?;

    // Format vector as SQL literal: '[0.1,0.2,...]'
    let embedding_sql = format!(
        "[{}]",
        embedding
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );

    // Fetch page_size + 1 to detect whether more rows exist
    let query = format!(
        r#"SELECT t.name AS tool_name,
                  t.description AS tool_description,
                  s.name AS server_name,
                  te.domain,
                  1 - (te.embedding <=> $1::vector) AS similarity
           FROM "{}" te
           JOIN mcp_server_tools t ON t.id = te.tool_id
           JOIN mcp_servers s ON s.id = te.server_id
           ORDER BY te.embedding <=> $1::vector
           LIMIT $2 OFFSET $3"#,
        model_config.tool_table_name
    );

    let rows = sqlx::query_as::<_, (String, String, String, String, f64)>(&query)
        .bind(&embedding_sql)
        .bind(page_size + 1)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| AppError::Internal(format!("Search query error: {e}")))?;

    let has_more = (rows.len() as i64) > page_size;
    let search_results: Vec<serde_json::Value> = rows
        .into_iter()
        .take(page_size as usize)
        .map(
            |(tool_name, tool_description, server_name, domain, similarity)| {
                serde_json::json!({
                    "toolName": tool_name,
                    "toolDescription": tool_description,
                    "serverName": server_name,
                    "domain": domain,
                    "similarity": similarity,
                })
            },
        )
        .collect();

    let next_token = if has_more {
        Some(encode_page_token(offset + page_size))
    } else {
        None
    };

    Ok(Json(serde_json::json!({
        "results": search_results,
        "nextToken": next_token,
        "query": body.query,
        "model": model_config.model,
        "provider": model_config.provider,
    })))
}

/// `POST /admin/embeddings/reindex` — re-index all server tools.
pub async fn reindex_handler(State(state): State<AppState>) -> AppResult<Json<serde_json::Value>> {
    let servers = nize_core::mcp::queries::list_all_servers(&state.pool)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list servers: {e}")))?;

    let mut indexed = 0usize;
    let mut errors: Vec<serde_json::Value> = Vec::new();

    for server in &servers {
        let server_id = server.id.to_string();
        match nize_core::embedding::indexer::embed_server_tools(
            &state.pool,
            &state.config_cache,
            &server_id,
            &state.config.mcp_encryption_key,
        )
        .await
        {
            Ok(count) => {
                indexed += count;
            }
            Err(e) => {
                errors.push(serde_json::json!({
                    "serverId": server_id,
                    "serverName": server.name,
                    "error": e.to_string(),
                }));
            }
        }
    }

    Ok(Json(serde_json::json!({
        "indexed": indexed,
        "serverCount": servers.len(),
        "errors": errors,
    })))
}
