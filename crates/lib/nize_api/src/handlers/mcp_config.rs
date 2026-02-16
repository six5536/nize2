// @zen-component: PLAN-017-McpConfigHandler
//
//! MCP server configuration request handlers.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthenticatedUser;
use crate::services::mcp_config;
use nize_core::models::mcp::ServerConfig;

// ---------------------------------------------------------------------------
// Request / response DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserServerRequest {
    pub name: String,
    pub description: Option<String>,
    pub domain: Option<String>,
    pub url: String,
    #[serde(default = "default_auth_type")]
    pub auth_type: String,
    pub api_key: Option<String>,
    pub api_key_header: Option<String>,
    pub headers: Option<serde_json::Value>,
}

fn default_auth_type() -> String {
    "none".to_string()
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserServerRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub domain: Option<String>,
    pub url: Option<String>,
    pub auth_type: Option<String>,
    pub api_key: Option<String>,
    pub api_key_header: Option<String>,
    pub headers: Option<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePreferenceRequest {
    pub enabled: bool,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionRequest {
    /// Transport configuration (discriminated union via `transport` field).
    pub config: ServerConfig,
    /// API key for testing (stored separately from config).
    pub api_key: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAdminServerRequest {
    pub name: String,
    pub description: Option<String>,
    pub domain: Option<String>,
    #[serde(default = "default_visible")]
    pub visibility: String,
    /// Transport configuration (discriminated union via `transport` field).
    pub config: ServerConfig,
    /// API key (stored separately in secrets).
    pub api_key: Option<String>,
}

fn default_visible() -> String {
    "visible".to_string()
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAdminServerRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub domain: Option<String>,
    pub visibility: Option<String>,
    pub enabled: Option<bool>,
    /// Updated transport configuration.
    pub config: Option<ServerConfig>,
    pub api_key: Option<String>,
}

// ---------------------------------------------------------------------------
// User MCP server endpoints
// ---------------------------------------------------------------------------

/// `GET /mcp/servers` — list user MCP servers.
pub async fn list_servers_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    let servers = mcp_config::get_servers_for_user(&state.pool, &user.0.sub).await?;
    Ok(Json(serde_json::json!({ "servers": servers })))
}

/// `POST /mcp/servers` — add user MCP server.
pub async fn add_server_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Json(body): Json<CreateUserServerRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let server = mcp_config::create_user_server(
        &state.pool,
        &user.0.sub,
        &body.name,
        body.description.as_deref().unwrap_or(""),
        body.domain.as_deref().unwrap_or("general"),
        &body.url,
        &body.auth_type,
        body.api_key.as_deref(),
        body.api_key_header.as_deref(),
        body.headers.as_ref(),
        &state.config.mcp_encryption_key,
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(server).unwrap()),
    ))
}

/// `PATCH /mcp/servers/{serverId}` — update user MCP server.
pub async fn update_server_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Path(server_id): Path<String>,
    Json(body): Json<UpdateUserServerRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let server = mcp_config::update_user_server(
        &state.pool,
        &user.0.sub,
        &server_id,
        body.name.as_deref(),
        body.description.as_deref(),
        body.domain.as_deref(),
        body.url.as_deref(),
        body.auth_type.as_deref(),
        body.api_key.as_deref(),
        body.api_key_header.as_deref(),
        body.headers.as_ref(),
        &state.config.mcp_encryption_key,
    )
    .await?;
    Ok(Json(serde_json::to_value(server).unwrap()))
}

/// `DELETE /mcp/servers/{serverId}` — remove user MCP server.
pub async fn delete_server_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Path(server_id): Path<String>,
) -> AppResult<StatusCode> {
    mcp_config::delete_user_server(&state.pool, &user.0.sub, &server_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `PATCH /mcp/servers/{serverId}/preference` — toggle server preference.
pub async fn update_preference_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Path(server_id): Path<String>,
    Json(body): Json<UpdatePreferenceRequest>,
) -> AppResult<StatusCode> {
    mcp_config::set_user_preference(&state.pool, &user.0.sub, &server_id, body.enabled).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /mcp/servers/{serverId}/tools` — list server tools.
pub async fn list_server_tools_handler(
    State(state): State<AppState>,
    Path(server_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let tools = mcp_config::get_server_tools(&state.pool, &server_id).await?;
    Ok(Json(serde_json::json!({ "tools": tools })))
}

// ---------------------------------------------------------------------------
// OAuth endpoints
// ---------------------------------------------------------------------------

/// `GET /mcp/servers/{serverId}/oauth/status` — get OAuth status (stub).
pub async fn oauth_status_handler(
    Path(_server_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    // OAuth flow implementation deferred
    Ok(Json(serde_json::json!({
        "connected": false
    })))
}

/// `POST /mcp/servers/{serverId}/oauth/initiate` — initiate OAuth flow (stub).
pub async fn oauth_initiate_handler(
    Path(_server_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    // OAuth flow implementation deferred
    Err(AppError::Validation(
        "OAuth flow not yet implemented".into(),
    ))
}

/// `POST /mcp/servers/{serverId}/oauth/revoke` — revoke OAuth token (stub).
pub async fn oauth_revoke_handler(Path(_server_id): Path<String>) -> StatusCode {
    StatusCode::NO_CONTENT
}

// ---------------------------------------------------------------------------
// Test connection
// ---------------------------------------------------------------------------

/// `POST /mcp/test-connection` — test MCP server connection.
pub async fn test_connection_handler(
    Json(body): Json<TestConnectionRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let result = mcp_config::test_connection(&body.config, body.api_key.as_deref()).await;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

// ---------------------------------------------------------------------------
// Admin MCP server endpoints
// ---------------------------------------------------------------------------

/// `GET /mcp/admin/servers` — list admin MCP servers.
pub async fn admin_list_servers_handler(
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let servers = mcp_config::get_all_servers(&state.pool).await?;
    Ok(Json(serde_json::json!({ "servers": servers })))
}

/// `POST /mcp/admin/servers` — create admin MCP server.
pub async fn admin_create_server_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Json(body): Json<CreateAdminServerRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let server = mcp_config::create_built_in_server(
        &state.pool,
        &user.0.sub,
        &body.name,
        body.description.as_deref().unwrap_or(""),
        body.domain.as_deref().unwrap_or("general"),
        &body.visibility,
        &body.config,
        body.api_key.as_deref(),
        &state.config.mcp_encryption_key,
    )
    .await?;

    // Discover and store tools from the server
    let test_result = mcp_config::test_connection(&body.config, body.api_key.as_deref()).await;
    if !test_result.tools.is_empty() {
        if let Err(e) =
            mcp_config::store_tools_from_test(&state.pool, &server.id, &test_result.tools).await
        {
            tracing::warn!("Failed to store tools for server {}: {e}", server.id);
        }

        // Generate embeddings for the newly stored tools
        if let Err(e) = nize_core::embedding::indexer::embed_server_tools(
            &state.pool,
            &state.config_cache,
            &server.id,
        )
        .await
        {
            tracing::warn!("Failed to embed tools for server {}: {e}", server.id);
        }
    }

    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(server).unwrap()),
    ))
}

/// `PATCH /mcp/admin/servers/{serverId}` — update admin MCP server.
pub async fn admin_update_server_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Path(server_id): Path<String>,
    Json(body): Json<UpdateAdminServerRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let server = mcp_config::update_built_in_server(
        &state.pool,
        &user.0.sub,
        &server_id,
        body.name.as_deref(),
        body.description.as_deref(),
        body.domain.as_deref(),
        body.visibility.as_deref(),
        body.enabled,
        body.config.as_ref(),
        body.api_key.as_deref(),
        &state.config.mcp_encryption_key,
    )
    .await?;

    // Re-discover and store tools when config changes
    if let Some(config) = &body.config {
        let test_result = mcp_config::test_connection(config, body.api_key.as_deref()).await;
        if !test_result.tools.is_empty() {
            if let Err(e) =
                mcp_config::store_tools_from_test(&state.pool, &server.id, &test_result.tools).await
            {
                tracing::warn!("Failed to store tools for server {}: {e}", server.id);
            }

            // Generate embeddings for the newly stored tools
            if let Err(e) = nize_core::embedding::indexer::embed_server_tools(
                &state.pool,
                &state.config_cache,
                &server.id,
            )
            .await
            {
                tracing::warn!("Failed to embed tools for server {}: {e}", server.id);
            }
        }
    }

    Ok(Json(serde_json::to_value(server).unwrap()))
}

/// `DELETE /mcp/admin/servers/{serverId}` — delete admin MCP server.
pub async fn admin_delete_server_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Path(server_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let result = mcp_config::delete_built_in_server(&state.pool, &user.0.sub, &server_id).await?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}
