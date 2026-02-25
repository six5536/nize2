// @awa-component: PLAN-017-McpConfigHandler
//
//! MCP server configuration request handlers.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthenticatedUser;
use crate::services::mcp_config;
use nize_core::mcp::execution::OAuthHeaders;
use nize_core::models::mcp::{OAuthConfig, ServerConfig, TransportType};

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
    // @awa-impl: XMCP-5_AC-1 — transport selector for user servers (http or sse only)
    #[serde(default = "default_transport")]
    pub transport: TransportType,
    #[serde(default = "default_auth_type")]
    pub auth_type: String,
    pub api_key: Option<String>,
    pub api_key_header: Option<String>,
    pub headers: Option<serde_json::Value>,
    pub oauth_config: Option<OAuthConfig>,
    pub client_secret: Option<String>,
}

fn default_transport() -> TransportType {
    TransportType::Http
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
    /// Server ID — required for OAuth so the backend can look up stored tokens.
    pub server_id: Option<String>,
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
    pub oauth_config: Option<OAuthConfig>,
    pub client_secret: Option<String>,
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
    pub oauth_config: Option<OAuthConfig>,
    pub client_secret: Option<String>,
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
        &body.transport,
        &body.auth_type,
        body.api_key.as_deref(),
        body.api_key_header.as_deref(),
        body.headers.as_ref(),
        body.oauth_config.as_ref(),
        body.client_secret.as_deref(),
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

/// `GET /mcp/servers/{serverId}/oauth/status` — get OAuth status.
// @awa-impl: PLAN-031 Phase 5.3
pub async fn oauth_status_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Path(server_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let has_token =
        nize_core::mcp::queries::has_valid_oauth_token(&state.pool, &user.0.sub, &server_id)
            .await
            .unwrap_or(false);

    // Check if token row exists (even if expired) — can refresh
    let token_row =
        nize_core::mcp::queries::get_oauth_token(&state.pool, &user.0.sub, &server_id).await?;

    let (connected, expires_at) = match token_row {
        Some(row) => {
            let exp = row.expires_at.to_rfc3339();
            (has_token, Some(exp))
        }
        None => (false, None),
    };

    Ok(Json(serde_json::json!({
        "connected": connected,
        "expiresAt": expires_at,
    })))
}

/// `POST /mcp/servers/{serverId}/oauth/initiate` — initiate OAuth flow.
// @awa-impl: PLAN-031 Phase 5.1
pub async fn oauth_initiate_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Path(server_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    use nize_core::mcp::oauth::{
        OAuthPendingState, compute_code_challenge, generate_code_verifier, generate_state,
    };

    // Load server to get OAuth config
    let server = nize_core::mcp::queries::get_server(&state.pool, &server_id)
        .await?
        .ok_or_else(|| AppError::Validation(format!("Server {server_id} not found")))?;

    let oauth_config_json = server
        .oauth_config
        .ok_or_else(|| AppError::Validation("Server has no OAuth configuration".into()))?;

    let oauth_config: nize_core::models::mcp::OAuthConfig =
        serde_json::from_value(oauth_config_json.clone())
            .map_err(|e| AppError::Validation(format!("Invalid OAuth config: {e}")))?;

    // Load and decrypt client_secret
    let encrypted_secret =
        nize_core::mcp::queries::get_oauth_client_secret_encrypted(&state.pool, &server_id)
            .await?
            .ok_or_else(|| {
                AppError::Validation("No OAuth client secret stored for server".into())
            })?;

    let client_secret =
        nize_core::mcp::secrets::decrypt(&encrypted_secret, &state.config.mcp_encryption_key)
            .map_err(|e| AppError::Internal(format!("Failed to decrypt client secret: {e}")))?;

    // Generate PKCE params
    let code_verifier = generate_code_verifier();
    let code_challenge = compute_code_challenge(&code_verifier);
    let state_param = generate_state();

    // Build redirect_uri from current API bind address
    let redirect_uri = format!(
        "http://{}{}{}",
        state.config.bind_addr,
        crate::API_PREFIX,
        crate::generated::routes::GET_AUTH_OAUTH_MCP_CALLBACK,
    );

    // Store pending state
    let pending = OAuthPendingState {
        server_id: server_id.clone(),
        user_id: user.0.sub.clone(),
        pkce_verifier: code_verifier,
        oauth_config_json,
        client_secret,
        redirect_uri: redirect_uri.clone(),
        created_at: std::time::Instant::now(),
    };
    state.oauth_state.insert(state_param.clone(), pending);

    // Build Google authorization URL
    let mut auth_url = url::Url::parse(&oauth_config.authorization_url)
        .map_err(|e| AppError::Validation(format!("Invalid authorization URL: {e}")))?;
    auth_url
        .query_pairs_mut()
        .append_pair("client_id", &oauth_config.client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", &oauth_config.scopes.join(" "))
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("state", &state_param)
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", "S256");

    Ok(Json(serde_json::json!({
        "authUrl": auth_url.as_str(),
    })))
}

/// `POST /mcp/servers/{serverId}/oauth/revoke` — revoke OAuth token.
// @awa-impl: PLAN-031 Phase 5.4
pub async fn oauth_revoke_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Path(server_id): Path<String>,
) -> AppResult<StatusCode> {
    // Load token for optional Google revocation
    let token_row =
        nize_core::mcp::queries::get_oauth_token(&state.pool, &user.0.sub, &server_id).await?;

    if let Some(row) = token_row {
        // Best-effort revoke at Google
        if let Ok(access_token) = nize_core::mcp::secrets::decrypt(
            &row.access_token_encrypted,
            &state.config.mcp_encryption_key,
        ) {
            let _ = reqwest::Client::new()
                .post("https://oauth2.googleapis.com/revoke")
                .form(&[("token", &access_token)])
                .send()
                .await;
        }
    }

    // Delete from DB
    nize_core::mcp::queries::delete_oauth_token(&state.pool, &user.0.sub, &server_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Test connection
// ---------------------------------------------------------------------------

/// `POST /mcp/test-connection` — test MCP server connection.
pub async fn test_connection_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Json(body): Json<TestConnectionRequest>,
) -> AppResult<Json<serde_json::Value>> {
    // Determine OAuth requirement for this test request.
    // Prefer server-level oauth_config when serverId is present so all transports
    // (including managed) share the same OAuth behavior.
    let server_uses_oauth = if let Some(sid) = &body.server_id {
        match nize_core::mcp::queries::get_server(&state.pool, sid).await? {
            Some(server) => server.oauth_config.is_some(),
            None => false,
        }
    } else {
        matches!(&body.config, ServerConfig::Http(http) if http.auth_type == "oauth")
            || matches!(&body.config, ServerConfig::Sse(sse) if sse.auth_type == "oauth")
    };

    // If OAuth is required, look up the stored OAuth headers for this user+server.
    let oauth_headers = if server_uses_oauth {
        if let Some(sid) = &body.server_id {
            match nize_core::mcp::queries::get_oauth_token(&state.pool, &user.0.sub, sid).await {
                Ok(Some(row)) => {
                    let id_token = match row.id_token_encrypted.as_deref() {
                        Some(encrypted) => match nize_core::mcp::secrets::decrypt(
                            encrypted,
                            &state.config.mcp_encryption_key,
                        ) {
                            Ok(token) => Some(token),
                            Err(e) => {
                                tracing::warn!("Failed to decrypt OAuth ID token for test: {e}");
                                None
                            }
                        },
                        None => None,
                    };

                    let access_token = match nize_core::mcp::secrets::decrypt(
                        &row.access_token_encrypted,
                        &state.config.mcp_encryption_key,
                    ) {
                        Ok(token) => Some(token),
                        Err(e) => {
                            tracing::warn!("Failed to decrypt OAuth access token for test: {e}");
                            None
                        }
                    };

                    match (id_token, access_token) {
                        (Some(id_token), Some(access_token)) => Some(OAuthHeaders {
                            id_token,
                            access_token,
                        }),
                        _ => None,
                    }
                }
                _ => None,
            }
        } else {
            None
        }
    } else {
        None
    };

    // If OAuth is required and no token is available, return authRequired
    // instead of attempting a connection.
    if server_uses_oauth && oauth_headers.is_none() {
        let result = nize_core::models::mcp::TestConnectionResult {
            success: false,
            error: Some("OAuth authorization required".to_string()),
            auth_required: Some(true),
            ..Default::default()
        };
        return Ok(Json(serde_json::to_value(result).unwrap()));
    }

    let result = mcp_config::test_connection(
        &body.config,
        body.api_key.as_deref(),
        oauth_headers.as_ref(),
    )
    .await;

    // When test succeeds and we know which server, persist discovered tools + embeddings
    if result.success && !result.tools.is_empty() {
        if let Some(ref server_id) = body.server_id {
            if let Err(e) =
                mcp_config::store_tools_from_test(&state.pool, server_id, &result.tools).await
            {
                tracing::warn!("Failed to store tools from test for server {server_id}: {e}");
            }

            // Mark server as available after successful connection
            if let Err(e) = nize_core::mcp::queries::update_server(
                &state.pool,
                server_id,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(true),
                None,
            )
            .await
            {
                tracing::warn!("Failed to set server available: {e}");
            }

            // Generate embeddings for the discovered tools
            if let Err(e) = nize_core::embedding::indexer::embed_server_tools(
                &state.pool,
                &state.config_cache,
                server_id,
                &state.config.mcp_encryption_key,
            )
            .await
            {
                tracing::warn!("Failed to embed tools for server {server_id}: {e}");
            }
        }
    }

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
        body.oauth_config.as_ref(),
        body.client_secret.as_deref(),
        &state.config.mcp_encryption_key,
    )
    .await?;

    // Discover and store tools from the server (skip for OAuth — no tokens yet)
    let is_oauth = match &body.config {
        ServerConfig::Http(http) => http.auth_type == "oauth",
        _ => false,
    };
    let test_result = if is_oauth {
        Default::default()
    } else {
        mcp_config::test_connection(&body.config, body.api_key.as_deref(), None).await
    };
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
            &state.config.mcp_encryption_key,
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
        body.oauth_config.as_ref(),
        body.client_secret.as_deref(),
        &state.config.mcp_encryption_key,
    )
    .await?;

    // Re-discover and store tools when config changes
    if let Some(config) = &body.config {
        // For OAuth servers, look up stored OAuth headers
        let oauth_headers = match config {
            ServerConfig::Http(http) if http.auth_type == "oauth" => {
                match nize_core::mcp::queries::get_oauth_token(&state.pool, &user.0.sub, &server_id)
                    .await
                {
                    Ok(Some(row)) => {
                        let id_token = match row.id_token_encrypted.as_deref() {
                            Some(encrypted) => match nize_core::mcp::secrets::decrypt(
                                encrypted,
                                &state.config.mcp_encryption_key,
                            ) {
                                Ok(token) => Some(token),
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed to decrypt OAuth ID token for tool discovery: {e}"
                                    );
                                    None
                                }
                            },
                            None => None,
                        };
                        let access_token = match nize_core::mcp::secrets::decrypt(
                            &row.access_token_encrypted,
                            &state.config.mcp_encryption_key,
                        ) {
                            Ok(token) => Some(token),
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to decrypt OAuth access token for tool discovery: {e}"
                                );
                                None
                            }
                        };
                        match (id_token, access_token) {
                            (Some(id_token), Some(access_token)) => Some(OAuthHeaders {
                                id_token,
                                access_token,
                            }),
                            _ => None,
                        }
                    }
                    _ => None,
                }
            }
            _ => None,
        };

        let test_result =
            mcp_config::test_connection(config, body.api_key.as_deref(), oauth_headers.as_ref())
                .await;
        if !test_result.tools.is_empty() {
            if let Err(e) =
                mcp_config::store_tools_from_test(&state.pool, &server.id, &test_result.tools).await
            {
                tracing::warn!("Failed to store tools for server {}: {e}", server.id);
            }

            // Mark server as available after successful tool discovery
            if let Err(e) = nize_core::mcp::queries::update_server(
                &state.pool,
                &server_id,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(true),
                None,
            )
            .await
            {
                tracing::warn!("Failed to set server available: {e}");
            }

            // Generate embeddings for the newly stored tools
            if let Err(e) = nize_core::embedding::indexer::embed_server_tools(
                &state.pool,
                &state.config_cache,
                &server.id,
                &state.config.mcp_encryption_key,
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
