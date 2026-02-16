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
    ServerConfig, ServerStatus, StdioServerConfig, TestConnectionResult, TransportType,
    UserServerView, VisibilityTier,
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

    let is_localhost = parsed.host_str().map_or(false, |h| {
        h == "localhost" || h == "127.0.0.1" || h == "::1"
    });

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
    encryption_key: &str,
) -> Result<UserServerView, McpError> {
    // Validate HTTP config
    validate_http_config(url, auth_type_str)?;

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

    // Insert server
    let server =
        queries::insert_user_server(pool, user_id, name, description, domain, &config, available)
            .await?;
    let server_id = server.id.to_string();

    // Store encrypted API key if provided
    if let Some(key) = api_key {
        if auth_type_str == "api-key" {
            let encrypted = nize_core::mcp::secrets::encrypt(key, encryption_key)?;
            queries::store_api_key(pool, &server_id, &encrypted, DEFAULT_ENCRYPTION_KEY_ID).await?;
        }
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
pub async fn create_built_in_server(
    pool: &PgPool,
    admin_id: &str,
    name: &str,
    description: &str,
    domain: &str,
    visibility: &str,
    config: &ServerConfig,
    api_key: Option<&str>,
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

    let server = queries::insert_built_in_server(
        pool,
        name,
        description,
        domain,
        &endpoint,
        &vis,
        &tp,
        Some(&config_json),
        None, // OAuth config
        true, // available
    )
    .await?;
    let server_id = server.id.to_string();

    // Store encrypted API key if provided
    if let Some(key) = api_key {
        let encrypted = nize_core::mcp::secrets::encrypt(key, encryption_key)?;
        queries::store_api_key(pool, &server_id, &encrypted, DEFAULT_ENCRYPTION_KEY_ID).await?;
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
    )
    .await?;

    // Store encrypted API key if provided
    if let Some(key) = api_key {
        let encrypted = nize_core::mcp::secrets::encrypt(key, encryption_key)?;
        queries::store_api_key(pool, server_id, &encrypted, DEFAULT_ENCRYPTION_KEY_ID).await?;
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
/// For HTTP transport: sends an MCP `initialize` JSON-RPC request.
/// For Stdio transport: spawns the process and performs a JSON-RPC handshake.
/// Returns server info and tool count on success.
pub async fn test_connection(config: &ServerConfig, api_key: Option<&str>) -> TestConnectionResult {
    match config {
        ServerConfig::Http(http) => test_connection_http(http, api_key).await,
        ServerConfig::Stdio(stdio) => test_connection_stdio(stdio).await,
    }
}

/// Test an HTTP MCP server connection.
async fn test_connection_http(
    config: &HttpServerConfig,
    api_key: Option<&str>,
) -> TestConnectionResult {
    let server_url = &config.url;
    if server_url.is_empty() {
        return TestConnectionResult {
            success: false,
            error: Some("URL is required for HTTP transport".into()),
            ..Default::default()
        };
    }

    // Build HTTP client request
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return TestConnectionResult {
                success: false,
                error: Some(format!("Failed to create HTTP client: {e}")),
                ..Default::default()
            };
        }
    };

    // MCP initialize request (JSON-RPC 2.0)
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "nize-mcp",
                "version": nize_core::version()
            }
        }
    });

    let mut req_builder = client.post(server_url).json(&init_request);

    // Add auth header if needed
    if config.auth_type == "api-key" {
        if let Some(key) = api_key {
            let header_name = config.api_key_header.as_deref().unwrap_or("X-API-Key");
            req_builder = req_builder.header(header_name, key);
        }
    }

    // Add custom headers
    if let Some(hdrs) = &config.headers {
        if let Some(map) = hdrs.as_object() {
            for (k, v) in map {
                if let Some(val) = v.as_str() {
                    req_builder = req_builder.header(k.as_str(), val);
                }
            }
        }
    }

    match req_builder.send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                return TestConnectionResult {
                    success: false,
                    error: Some(format!("Server returned HTTP {}", resp.status())),
                    error_details: resp.text().await.ok(),
                    ..Default::default()
                };
            }

            match resp.json::<serde_json::Value>().await {
                Ok(body) => {
                    // Parse MCP initialize response
                    let result = body.get("result");
                    let server_info = result.and_then(|r| r.get("serverInfo"));
                    let server_name = server_info
                        .and_then(|s| s.get("name"))
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string());
                    let server_version = server_info
                        .and_then(|s| s.get("version"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let protocol_version = result
                        .and_then(|r| r.get("protocolVersion"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    // Now try to list tools
                    let tools_request = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": 2,
                        "method": "tools/list",
                        "params": {}
                    });

                    let mut tools_req = client.post(server_url).json(&tools_request);

                    if config.auth_type == "api-key" {
                        if let Some(key) = api_key {
                            let header_name =
                                config.api_key_header.as_deref().unwrap_or("X-API-Key");
                            tools_req = tools_req.header(header_name, key);
                        }
                    }
                    if let Some(hdrs) = &config.headers {
                        if let Some(map) = hdrs.as_object() {
                            for (k, v) in map {
                                if let Some(val) = v.as_str() {
                                    tools_req = tools_req.header(k.as_str(), val);
                                }
                            }
                        }
                    }

                    let tools = match tools_req.send().await {
                        Ok(tools_resp) => {
                            if let Ok(tools_body) = tools_resp.json::<serde_json::Value>().await {
                                parse_tools_response(&tools_body)
                            } else {
                                vec![]
                            }
                        }
                        Err(_) => vec![],
                    };

                    let tool_count = tools.len() as i64;

                    TestConnectionResult {
                        success: true,
                        server_name,
                        server_version,
                        protocol_version,
                        tool_count: Some(tool_count),
                        error: None,
                        error_details: None,
                        tools,
                    }
                }
                Err(e) => TestConnectionResult {
                    success: false,
                    error: Some(format!("Invalid JSON response: {e}")),
                    ..Default::default()
                },
            }
        }
        Err(e) => {
            let error = if e.is_timeout() {
                "Connection timed out (10s)".to_string()
            } else if e.is_connect() {
                "Connection refused".to_string()
            } else {
                format!("Connection failed: {e}")
            };
            TestConnectionResult {
                success: false,
                error: Some(error),
                ..Default::default()
            }
        }
    }
}

/// Test a stdio MCP server connection by spawning the process and
/// performing an MCP JSON-RPC handshake over stdin/stdout.
async fn test_connection_stdio(config: &StdioServerConfig) -> TestConnectionResult {
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::process::Command;

    if config.command.is_empty() {
        return TestConnectionResult {
            success: false,
            error: Some("Command is required for stdio transport".into()),
            ..Default::default()
        };
    }

    let args = config.args.as_deref().unwrap_or_default();

    let mut cmd = Command::new(&config.command);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Set environment variables if provided
    if let Some(env) = &config.env {
        for (k, v) in env {
            cmd.env(k, v);
        }
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return TestConnectionResult {
                success: false,
                error: Some(format!("Failed to spawn process '{}': {e}", config.command)),
                ..Default::default()
            };
        }
    };

    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();

    // MCP initialize request (JSON-RPC 2.0)
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "nize-mcp",
                "version": nize_core::version()
            }
        }
    });

    let result = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut writer = stdin;
        let mut reader = BufReader::new(stdout);

        // Send initialize
        let msg = serde_json::to_string(&init_request).unwrap();
        writer.write_all(msg.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        // Read initialize response
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        let init_resp: serde_json::Value = serde_json::from_str(line.trim())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        // Send initialized notification
        let initialized = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });
        let msg = serde_json::to_string(&initialized).unwrap();
        writer.write_all(msg.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        // Send tools/list
        let tools_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        });
        let msg = serde_json::to_string(&tools_request).unwrap();
        writer.write_all(msg.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        // Read tools/list response
        let mut tools_line = String::new();
        reader.read_line(&mut tools_line).await?;
        let tools_resp: serde_json::Value = serde_json::from_str(tools_line.trim())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok::<_, std::io::Error>((init_resp, tools_resp))
    })
    .await;

    // Kill the child process
    let _ = child.kill().await;

    match result {
        Ok(Ok((init_resp, tools_resp))) => {
            let result_val = init_resp.get("result");
            let server_info = result_val.and_then(|r| r.get("serverInfo"));
            let server_name = server_info
                .and_then(|s| s.get("name"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());
            let server_version = server_info
                .and_then(|s| s.get("version"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let protocol_version = result_val
                .and_then(|r| r.get("protocolVersion"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let tools = parse_tools_response(&tools_resp);
            let tool_count = tools.len() as i64;

            TestConnectionResult {
                success: true,
                server_name,
                server_version,
                protocol_version,
                tool_count: Some(tool_count),
                error: None,
                error_details: None,
                tools,
            }
        }
        Ok(Err(e)) => TestConnectionResult {
            success: false,
            error: Some(format!("Stdio communication error: {e}")),
            ..Default::default()
        },
        Err(_) => TestConnectionResult {
            success: false,
            error: Some("Connection timed out (15s)".into()),
            ..Default::default()
        },
    }
}

/// Parse tools from an MCP tools/list response.
fn parse_tools_response(body: &serde_json::Value) -> Vec<McpToolSummary> {
    body.get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|tool| {
                    let name = tool.get("name")?.as_str()?.to_string();
                    let description = tool
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("")
                        .to_string();
                    Some(McpToolSummary { name, description })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Store tools from a test connection result for a server.
pub async fn store_tools_from_test(
    pool: &PgPool,
    server_id: &str,
    tools: &[McpToolSummary],
) -> Result<(), McpError> {
    queries::replace_server_tools(pool, server_id, tools).await
}
