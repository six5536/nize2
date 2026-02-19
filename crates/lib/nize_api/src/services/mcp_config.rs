//! MCP server configuration service.
//!
//! Business logic for managing MCP server registrations, user preferences,
//! and connection testing. Ported from reference project's ConfigService.

use sqlx::PgPool;
use tracing::{error, info};

use nize_core::mcp::McpError;
use nize_core::mcp::queries;
use nize_core::models::mcp::{
    AdminServerView, AuthType, DeleteResult, HttpServerConfig, McpServerRow, McpToolSummary,
    OAuthConfig, ServerConfig, ServerStatus, TestConnectionResult, TransportType, UserServerView,
    VisibilityTier,
};

/// Maximum number of user-owned servers.
const USER_SERVER_LIMIT: usize = 10;

/// Default encryption key ID.
const DEFAULT_ENCRYPTION_KEY_ID: &str = "v1";

// =============================================================================
// Validation helpers
// =============================================================================

/// Validate an HTTP server config.
fn validate_http_config(url: &str, auth_type_str: &str) -> Result<(), McpError> {
    if url.trim().is_empty() {
        return Err(McpError::InvalidTransport(
            "HTTP config requires a non-empty URL".into(),
        ));
    }

    let parsed = url::Url::parse(url)
        .map_err(|_| McpError::InvalidTransport(format!("Invalid URL format: {url}")))?;

    let is_localhost = parsed
        .host_str()
        .is_some_and(|h| h == "localhost" || h == "127.0.0.1" || h == "::1");

    if parsed.scheme() != "https" && !is_localhost {
        return Err(McpError::InvalidTransport(
            "HTTP URL must use HTTPS (HTTP only allowed for localhost)".into(),
        ));
    }

    if !["none", "api-key", "oauth"].contains(&auth_type_str) {
        return Err(McpError::InvalidTransport(format!(
            "Invalid authType: {auth_type_str}"
        )));
    }

    Ok(())
}

/// Compute server status for a user.
async fn compute_status(
    pool: &PgPool,
    server: &McpServerRow,
    user_id: &str,
    user_pref: Option<bool>,
) -> ServerStatus {
    if !server.available {
        return ServerStatus::Unavailable;
    }

    if user_pref == Some(false) {
        return ServerStatus::Disabled;
    }

    let auth_type = queries::extract_auth_type(&server.config);
    if auth_type == AuthType::OAuth {
        let has_token = queries::has_valid_oauth_token(pool, user_id, &server.id.to_string())
            .await
            .unwrap_or(false);
        if !has_token {
            return ServerStatus::Unauthorized;
        }
    }

    ServerStatus::Enabled
}

/// Convert a McpServerRow to UserServerView.
async fn to_user_view(
    pool: &PgPool,
    server: &McpServerRow,
    user_id: &str,
    user_pref: Option<bool>,
) -> Result<UserServerView, McpError> {
    let tool_count = queries::get_tool_count(pool, &server.id.to_string()).await?;
    let status = compute_status(pool, server, user_id, user_pref).await;

    Ok(UserServerView {
        id: server.id.to_string(),
        name: server.name.clone(),
        description: server.description.clone(),
        domain: server.domain.clone(),
        visibility: server.visibility.clone(),
        status,
        tool_count,
        is_owned: server
            .owner_id
            .map(|o| o.to_string() == user_id)
            .unwrap_or(false),
        created_at: server.created_at.to_rfc3339(),
        updated_at: server.updated_at.to_rfc3339(),
    })
}

/// Convert a McpServerRow to AdminServerView.
async fn to_admin_view(pool: &PgPool, server: &McpServerRow) -> Result<AdminServerView, McpError> {
    let tool_count = queries::get_tool_count(pool, &server.id.to_string()).await?;
    let user_preference_count =
        queries::get_user_preference_count(pool, &server.id.to_string()).await?;
    let auth_type = queries::extract_auth_type(&server.config);

    // For admin view, status is simple: enabled if available, unavailable otherwise
    let status = if !server.available {
        ServerStatus::Unavailable
    } else if !server.enabled {
        ServerStatus::Disabled
    } else {
        ServerStatus::Enabled
    };

    Ok(AdminServerView {
        id: server.id.to_string(),
        name: server.name.clone(),
        description: server.description.clone(),
        domain: server.domain.clone(),
        visibility: server.visibility.clone(),
        status,
        tool_count,
        is_owned: server.owner_id.is_some(),
        transport: server.transport.clone(),
        auth_type,
        owner_id: server.owner_id.map(|o| o.to_string()),
        user_preference_count,
        enabled: server.enabled,
        available: server.available,
        config: server.config.clone(),
        oauth_config: server.oauth_config.clone(),
        created_at: server.created_at.to_rfc3339(),
        updated_at: server.updated_at.to_rfc3339(),
    })
}

// =============================================================================
// User operations
// =============================================================================

/// List servers visible to a user.
pub async fn get_servers_for_user(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<UserServerView>, McpError> {
    let servers = queries::list_servers_for_user(pool, user_id).await?;
    let prefs = queries::get_user_preferences(pool, user_id).await?;
    let pref_map: std::collections::HashMap<_, _> = prefs
        .iter()
        .map(|p| (p.server_id.to_string(), p.enabled))
        .collect();

    let mut views = Vec::with_capacity(servers.len());
    for server in &servers {
        let user_pref = pref_map.get(&server.id.to_string()).copied();
        let view = to_user_view(pool, server, user_id, user_pref).await?;
        views.push(view);
    }
    Ok(views)
}

/// Create a new user MCP server.
#[allow(clippy::too_many_arguments)]
pub async fn create_user_server(
    pool: &PgPool,
    user_id: &str,
    name: &str,
    description: &str,
    domain: &str,
    url: &str,
    auth_type_str: &str,
    api_key: Option<&str>,
    api_key_header: Option<&str>,
    headers: Option<&serde_json::Value>,
    oauth_config: Option<&OAuthConfig>,
    client_secret: Option<&str>,
    encryption_key: &str,
) -> Result<UserServerView, McpError> {
    // Validate HTTP config
    validate_http_config(url, auth_type_str)?;

    // Validate OAuth fields when auth_type is "oauth"
    if auth_type_str == "oauth" {
        if oauth_config.is_none() {
            return Err(McpError::Validation(
                "oauthConfig is required when authType is 'oauth'".into(),
            ));
        }
        if client_secret.is_none() {
            return Err(McpError::Validation(
                "clientSecret is required when authType is 'oauth'".into(),
            ));
        }
        // Validate scopes include required openid + email
        if let Some(cfg) = oauth_config {
            if !cfg.scopes.iter().any(|s| s == "openid") {
                return Err(McpError::Validation(
                    "OAuth scopes must include 'openid'".into(),
                ));
            }
            if !cfg.scopes.iter().any(|s| s == "email") {
                return Err(McpError::Validation(
                    "OAuth scopes must include 'email'".into(),
                ));
            }
        }
    }

    // Check server limit
    let count = queries::count_user_servers(pool, user_id).await?;
    if count >= USER_SERVER_LIMIT as i64 {
        return Err(McpError::ServerLimitExceeded(USER_SERVER_LIMIT));
    }

    // Check duplicate name
    if queries::user_has_server_named(pool, user_id, name).await? {
        return Err(McpError::DuplicateServer(name.to_string()));
    }

    // Build config
    let config = ServerConfig::Http(HttpServerConfig {
        url: url.to_string(),
        headers: headers.cloned(),
        auth_type: auth_type_str.to_string(),
        api_key_header: api_key_header.map(|s| s.to_string()),
    });

    // Determine availability (OAuth servers need auth first)
    let available = auth_type_str != "oauth";

    // Serialize oauth_config if provided
    let oauth_config_json = oauth_config
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| McpError::Validation(format!("Failed to serialize oauth_config: {e}")))?;

    // Insert server
    let server = queries::insert_user_server(
        pool,
        user_id,
        name,
        description,
        domain,
        &config,
        oauth_config_json.as_ref(),
        available,
    )
    .await?;
    let server_id = server.id.to_string();

    // Store encrypted API key if provided
    if let Some(key) = api_key
        && auth_type_str == "api-key"
    {
        let encrypted = nize_core::mcp::secrets::encrypt(key, encryption_key)?;
        queries::store_api_key(pool, &server_id, &encrypted, DEFAULT_ENCRYPTION_KEY_ID).await?;
    }

    // Store encrypted OAuth client secret if provided
    if let Some(secret) = client_secret
        && auth_type_str == "oauth"
    {
        let encrypted = nize_core::mcp::secrets::encrypt(secret, encryption_key)?;
        queries::store_oauth_client_secret(pool, &server_id, &encrypted, DEFAULT_ENCRYPTION_KEY_ID)
            .await?;
    }

    // Log audit
    let details = serde_json::json!({
        "visibility": "user",
        "transport": "http",
        "domain": domain,
    });
    if let Err(e) = queries::insert_audit_log(
        pool,
        user_id,
        Some(&server_id),
        name,
        "created",
        Some(&details),
    )
    .await
    {
        error!("Failed to write audit log: {e}");
    }

    info!(server_id = %server_id, "Created user MCP server: {name}");

    to_user_view(pool, &server, user_id, None).await
}

/// Update a user MCP server.
#[allow(clippy::too_many_arguments)]
pub async fn update_user_server(
    pool: &PgPool,
    user_id: &str,
    server_id: &str,
    name: Option<&str>,
    description: Option<&str>,
    domain: Option<&str>,
    url: Option<&str>,
    auth_type_str: Option<&str>,
    api_key: Option<&str>,
    api_key_header: Option<&str>,
    headers: Option<&serde_json::Value>,
    encryption_key: &str,
) -> Result<UserServerView, McpError> {
    // Verify server exists and is owned by user
    let existing = queries::get_server(pool, server_id)
        .await?
        .ok_or_else(|| McpError::NotFound(format!("Server {server_id} not found")))?;

    if existing.visibility != VisibilityTier::User
        || existing
            .owner_id
            .map(|o| o.to_string() != user_id)
            .unwrap_or(true)
    {
        return Err(McpError::Forbidden(
            "Cannot modify a server you don't own".into(),
        ));
    }

    // Validate URL if provided
    if let Some(u) = url {
        let at = auth_type_str.unwrap_or("none");
        validate_http_config(u, at)?;
    }

    // Build config update if URL or auth fields changed
    let config_json = if url.is_some() || auth_type_str.is_some() || headers.is_some() {
        let current_http: HttpServerConfig = existing
            .config
            .as_ref()
            .and_then(|c| {
                serde_json::from_value::<ServerConfig>(c.clone())
                    .ok()
                    .and_then(|sc| match sc {
                        ServerConfig::Http(h) => Some(h),
                        _ => None,
                    })
            })
            .unwrap_or(HttpServerConfig {
                url: existing.endpoint.clone(),
                headers: None,
                auth_type: "none".to_string(),
                api_key_header: None,
            });

        let new_config = ServerConfig::Http(HttpServerConfig {
            url: url.unwrap_or(&current_http.url).to_string(),
            headers: headers.cloned().or(current_http.headers),
            auth_type: auth_type_str.unwrap_or(&current_http.auth_type).to_string(),
            api_key_header: api_key_header
                .map(|s| s.to_string())
                .or(current_http.api_key_header),
        });
        Some(serde_json::to_value(&new_config).unwrap())
    } else {
        None
    };

    let server = queries::update_server(
        pool,
        server_id,
        name,
        description,
        domain,
        url,
        config_json.as_ref(),
        None,
        None,
        None,
        None,
    )
    .await?;

    // Store encrypted API key if provided
    if let Some(key) = api_key {
        let encrypted = nize_core::mcp::secrets::encrypt(key, encryption_key)?;
        queries::store_api_key(pool, server_id, &encrypted, DEFAULT_ENCRYPTION_KEY_ID).await?;
    }

    // Audit
    let details = serde_json::json!({ "action": "user_update" });
    if let Err(e) = queries::insert_audit_log(
        pool,
        user_id,
        Some(server_id),
        &server.name,
        "updated",
        Some(&details),
    )
    .await
    {
        error!("Failed to write audit log: {e}");
    }

    to_user_view(pool, &server, user_id, None).await
}

/// Delete a user MCP server.
pub async fn delete_user_server(
    pool: &PgPool,
    user_id: &str,
    server_id: &str,
) -> Result<(), McpError> {
    let existing = queries::get_server(pool, server_id)
        .await?
        .ok_or_else(|| McpError::NotFound(format!("Server {server_id} not found")))?;

    if existing.visibility != VisibilityTier::User
        || existing
            .owner_id
            .map(|o| o.to_string() != user_id)
            .unwrap_or(true)
    {
        return Err(McpError::Forbidden(
            "Cannot delete a server you don't own".into(),
        ));
    }

    let name = existing.name.clone();
    queries::delete_server(pool, server_id).await?;

    // Audit
    let details = serde_json::json!({ "action": "user_delete" });
    if let Err(e) =
        queries::insert_audit_log(pool, user_id, None, &name, "deleted", Some(&details)).await
    {
        error!("Failed to write audit log: {e}");
    }

    info!(server_id = %server_id, "Deleted user MCP server: {name}");
    Ok(())
}

/// Toggle a user's preference for a server.
pub async fn set_user_preference(
    pool: &PgPool,
    user_id: &str,
    server_id: &str,
    enabled: bool,
) -> Result<(), McpError> {
    // Verify server exists
    queries::get_server(pool, server_id)
        .await?
        .ok_or_else(|| McpError::NotFound(format!("Server {server_id} not found")))?;

    queries::set_user_preference(pool, user_id, server_id, enabled).await
}

/// Get tools for a server.
pub async fn get_server_tools(
    pool: &PgPool,
    server_id: &str,
) -> Result<Vec<McpToolSummary>, McpError> {
    // Verify server exists
    queries::get_server(pool, server_id)
        .await?
        .ok_or_else(|| McpError::NotFound(format!("Server {server_id} not found")))?;

    let tool_rows = queries::list_server_tools(pool, server_id).await?;
    Ok(tool_rows
        .into_iter()
        .map(|t| McpToolSummary {
            name: t.name,
            description: t.description,
        })
        .collect())
}

// =============================================================================
// Admin operations
// =============================================================================

/// List all servers (admin).
pub async fn get_all_servers(pool: &PgPool) -> Result<Vec<AdminServerView>, McpError> {
    let servers = queries::list_all_servers(pool).await?;
    let mut views = Vec::with_capacity(servers.len());
    for server in &servers {
        let view = to_admin_view(pool, server).await?;
        views.push(view);
    }
    Ok(views)
}

/// Create a built-in server (admin).
#[allow(clippy::too_many_arguments)]
pub async fn create_built_in_server(
    pool: &PgPool,
    admin_id: &str,
    name: &str,
    description: &str,
    domain: &str,
    visibility: &str,
    config: &ServerConfig,
    api_key: Option<&str>,
    oauth_config: Option<&OAuthConfig>,
    client_secret: Option<&str>,
    encryption_key: &str,
) -> Result<AdminServerView, McpError> {
    let vis = match visibility {
        "hidden" => VisibilityTier::Hidden,
        "visible" => VisibilityTier::Visible,
        _ => {
            return Err(McpError::Validation(format!(
                "Invalid visibility: {visibility}"
            )));
        }
    };

    let tp = config.transport_type();
    let endpoint = config.endpoint().to_string();

    // Validate HTTP config if applicable
    if let ServerConfig::Http(http) = config {
        validate_http_config(&http.url, &http.auth_type)?;
    }

    // Serialize config to JSON (includes transport tag)
    let config_json = serde_json::to_value(config)
        .map_err(|e| McpError::Validation(format!("Failed to serialize config: {e}")))?;

    // Serialize oauth_config if provided
    let oauth_config_json = oauth_config
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| McpError::Validation(format!("Failed to serialize oauth_config: {e}")))?;

    // OAuth servers start as unavailable until user authorizes
    let auth_type_str = if let ServerConfig::Http(http) = config {
        http.auth_type.as_str()
    } else {
        "none"
    };
    let available = auth_type_str != "oauth";

    let server = queries::insert_built_in_server(
        pool,
        name,
        description,
        domain,
        &endpoint,
        &vis,
        &tp,
        Some(&config_json),
        oauth_config_json.as_ref(),
        available,
    )
    .await?;
    let server_id = server.id.to_string();

    // Store encrypted API key if provided
    if let Some(key) = api_key {
        let encrypted = nize_core::mcp::secrets::encrypt(key, encryption_key)?;
        queries::store_api_key(pool, &server_id, &encrypted, DEFAULT_ENCRYPTION_KEY_ID).await?;
    }

    // Store encrypted OAuth client secret if provided
    if let Some(secret) = client_secret {
        let encrypted = nize_core::mcp::secrets::encrypt(secret, encryption_key)?;
        queries::store_oauth_client_secret(pool, &server_id, &encrypted, DEFAULT_ENCRYPTION_KEY_ID)
            .await?;
    }

    // Audit
    let transport_str = match &tp {
        TransportType::Http => "http",
        TransportType::Stdio => "stdio",
    };
    let details = serde_json::json!({
        "visibility": visibility,
        "transport": transport_str,
        "domain": domain,
    });
    if let Err(e) = queries::insert_audit_log(
        pool,
        admin_id,
        Some(&server_id),
        name,
        "created",
        Some(&details),
    )
    .await
    {
        error!("Failed to write audit log: {e}");
    }

    info!(server_id = %server_id, "Created built-in MCP server: {name}");
    to_admin_view(pool, &server).await
}

/// Update a built-in server (admin).
#[allow(clippy::too_many_arguments)]
pub async fn update_built_in_server(
    pool: &PgPool,
    admin_id: &str,
    server_id: &str,
    name: Option<&str>,
    description: Option<&str>,
    domain: Option<&str>,
    visibility: Option<&str>,
    enabled: Option<bool>,
    config: Option<&ServerConfig>,
    api_key: Option<&str>,
    oauth_config: Option<&OAuthConfig>,
    client_secret: Option<&str>,
    encryption_key: &str,
) -> Result<AdminServerView, McpError> {
    // Verify server exists and is not user-owned
    let existing = queries::get_server(pool, server_id)
        .await?
        .ok_or_else(|| McpError::NotFound(format!("Server {server_id} not found")))?;

    if existing.visibility == VisibilityTier::User {
        return Err(McpError::Forbidden(
            "Cannot admin-edit a user-owned server".into(),
        ));
    }

    let vis = visibility
        .map(|v| match v {
            "hidden" => Ok(VisibilityTier::Hidden),
            "visible" => Ok(VisibilityTier::Visible),
            _ => Err(McpError::Validation(format!("Invalid visibility: {v}"))),
        })
        .transpose()?;

    // Validate HTTP config if provided
    if let Some(ServerConfig::Http(http)) = config {
        validate_http_config(&http.url, &http.auth_type)?;
    }

    // Build config JSON from provided config or leave unchanged
    let config_json = config.map(|c| serde_json::to_value(c).unwrap());
    let endpoint = config.map(|c| c.endpoint());

    // Serialize oauth_config if provided
    let oauth_config_json = oauth_config
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| McpError::Validation(format!("Failed to serialize oauth_config: {e}")))?;

    let server = queries::update_server(
        pool,
        server_id,
        name,
        description,
        domain,
        endpoint,
        config_json.as_ref(),
        enabled,
        vis.as_ref(),
        None,
        oauth_config_json.as_ref(),
    )
    .await?;

    // Store encrypted API key if provided
    if let Some(key) = api_key {
        let encrypted = nize_core::mcp::secrets::encrypt(key, encryption_key)?;
        queries::store_api_key(pool, server_id, &encrypted, DEFAULT_ENCRYPTION_KEY_ID).await?;
    }

    // Store encrypted OAuth client secret if provided
    if let Some(secret) = client_secret {
        let encrypted = nize_core::mcp::secrets::encrypt(secret, encryption_key)?;
        queries::store_oauth_client_secret(pool, server_id, &encrypted, DEFAULT_ENCRYPTION_KEY_ID)
            .await?;
    }

    // Invalidate all user OAuth tokens when OAuth config actually changes
    let oauth_config_changed = match (oauth_config, &existing.oauth_config) {
        (Some(new_cfg), Some(existing_json)) => {
            match serde_json::from_value::<OAuthConfig>(existing_json.clone()) {
                Ok(existing_cfg) => new_cfg != &existing_cfg,
                Err(_) => true, // Can't parse existing — treat as changed
            }
        }
        (Some(_), None) => true, // Adding OAuth config where none existed
        (None, _) => false,      // No new config provided
    };
    if oauth_config_changed || client_secret.is_some() {
        let revoked = queries::delete_all_oauth_tokens_for_server(pool, server_id).await?;
        if revoked > 0 {
            info!(server_id = %server_id, revoked = revoked, "Revoked OAuth tokens after config change");
        }
    }

    // Audit
    let details = serde_json::json!({ "action": "admin_update" });
    if let Err(e) = queries::insert_audit_log(
        pool,
        admin_id,
        Some(server_id),
        &server.name,
        "updated",
        Some(&details),
    )
    .await
    {
        error!("Failed to write audit log: {e}");
    }

    to_admin_view(pool, &server).await
}

/// Delete a built-in server (admin).
pub async fn delete_built_in_server(
    pool: &PgPool,
    admin_id: &str,
    server_id: &str,
) -> Result<DeleteResult, McpError> {
    let existing = queries::get_server(pool, server_id)
        .await?
        .ok_or_else(|| McpError::NotFound(format!("Server {server_id} not found")))?;

    if existing.visibility == VisibilityTier::User {
        return Err(McpError::Forbidden(
            "Cannot admin-delete a user-owned server. The owner must delete it.".into(),
        ));
    }

    let affected_users = queries::get_user_preference_count(pool, server_id).await?;
    let name = existing.name.clone();

    queries::delete_server(pool, server_id).await?;

    // Audit
    let details = serde_json::json!({ "action": "admin_delete", "affectedUsers": affected_users });
    if let Err(e) =
        queries::insert_audit_log(pool, admin_id, None, &name, "deleted", Some(&details)).await
    {
        error!("Failed to write audit log: {e}");
    }

    info!(server_id = %server_id, "Admin deleted MCP server: {name}");

    let warning = if affected_users > 0 {
        Some(format!("{affected_users} user(s) had this server enabled"))
    } else {
        None
    };

    Ok(DeleteResult {
        deleted: true,
        warning,
        affected_users: Some(affected_users),
    })
}

// =============================================================================
// Connection testing
// =============================================================================

/// Test connection to an MCP server.
///
/// For HTTP transport: connects via rmcp StreamableHttp transport.
/// For Stdio transport: spawns the process and performs a JSON-RPC handshake.
/// Returns server info and tool count on success.
pub async fn test_connection(
    config: &ServerConfig,
    api_key: Option<&str>,
    oauth_token: Option<&str>,
) -> TestConnectionResult {
    match config {
        ServerConfig::Http(http) => {
            nize_core::mcp::execution::test_http_connection(http, api_key, oauth_token).await
        }
        ServerConfig::Stdio(stdio) => nize_core::mcp::execution::test_stdio_connection(stdio).await,
    }
}

/// Store tools from a test connection result for a server.
pub async fn store_tools_from_test(
    pool: &PgPool,
    server_id: &str,
    tools: &[McpToolSummary],
) -> Result<(), McpError> {
    queries::replace_server_tools(pool, server_id, tools).await
}
