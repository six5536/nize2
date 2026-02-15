//! MCP server registry database queries.
//!
//! Raw SQLx queries for CRUD operations on MCP tables.

use sqlx::PgPool;

use super::McpError;
use crate::models::mcp::{
    AuthType, HttpServerConfig, McpServerRow, McpServerToolRow, McpToolSummary, TransportType,
    UserMcpPreferenceRow, VisibilityTier,
};

// =============================================================================
// Server queries
// =============================================================================

/// List servers visible to a user (visibility=visible OR owner's user servers).
pub async fn list_servers_for_user(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<McpServerRow>, McpError> {
    let rows = sqlx::query_as::<_, McpServerRow>(
        r#"
        SELECT id, name, description, domain, endpoint,
               visibility, transport, config, oauth_config,
               default_response_size_limit, owner_id,
               enabled, available, created_at, updated_at
        FROM mcp_servers
        WHERE enabled = true
          AND (
            visibility = 'visible'
            OR (visibility = 'user' AND owner_id = $1::uuid)
          )
        ORDER BY name
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// List all servers (admin view).
pub async fn list_all_servers(pool: &PgPool) -> Result<Vec<McpServerRow>, McpError> {
    let rows = sqlx::query_as::<_, McpServerRow>(
        r#"
        SELECT id, name, description, domain, endpoint,
               visibility, transport, config, oauth_config,
               default_response_size_limit, owner_id,
               enabled, available, created_at, updated_at
        FROM mcp_servers
        ORDER BY visibility, name
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Get a single server by ID.
pub async fn get_server(pool: &PgPool, server_id: &str) -> Result<Option<McpServerRow>, McpError> {
    let row = sqlx::query_as::<_, McpServerRow>(
        r#"
        SELECT id, name, description, domain, endpoint,
               visibility, transport, config, oauth_config,
               default_response_size_limit, owner_id,
               enabled, available, created_at, updated_at
        FROM mcp_servers
        WHERE id = $1::uuid
        "#,
    )
    .bind(server_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Count user-owned servers.
pub async fn count_user_servers(pool: &PgPool, user_id: &str) -> Result<i64, McpError> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM mcp_servers WHERE visibility = 'user' AND owner_id = $1::uuid",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Check if a user already has a server with the given name.
pub async fn user_has_server_named(
    pool: &PgPool,
    user_id: &str,
    name: &str,
) -> Result<bool, McpError> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM mcp_servers
            WHERE name = $1 AND owner_id = $2::uuid AND visibility = 'user'
        )
        "#,
    )
    .bind(name)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

/// Insert a new user server (visibility=user).
pub async fn insert_user_server(
    pool: &PgPool,
    user_id: &str,
    name: &str,
    description: &str,
    domain: &str,
    config: &HttpServerConfig,
    available: bool,
) -> Result<McpServerRow, McpError> {
    let config_json = serde_json::to_value(config)
        .map_err(|e| McpError::Validation(format!("Failed to serialize config: {e}")))?;

    let row = sqlx::query_as::<_, McpServerRow>(
        r#"
        INSERT INTO mcp_servers (name, description, domain, endpoint, visibility, transport, config, owner_id, enabled, available)
        VALUES ($1, $2, $3, $4, 'user', 'http', $5, $6::uuid, true, $7)
        RETURNING id, name, description, domain, endpoint,
                  visibility, transport, config, oauth_config,
                  default_response_size_limit, owner_id,
                  enabled, available, created_at, updated_at
        "#,
    )
    .bind(name)
    .bind(description)
    .bind(domain)
    .bind(&config.url)
    .bind(&config_json)
    .bind(user_id)
    .bind(available)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Insert a built-in server (admin).
pub async fn insert_built_in_server(
    pool: &PgPool,
    name: &str,
    description: &str,
    domain: &str,
    endpoint: &str,
    visibility: &VisibilityTier,
    transport: &TransportType,
    config: Option<&serde_json::Value>,
    oauth_config: Option<&serde_json::Value>,
    available: bool,
) -> Result<McpServerRow, McpError> {
    let row = sqlx::query_as::<_, McpServerRow>(
        r#"
        INSERT INTO mcp_servers (name, description, domain, endpoint, visibility, transport, config, oauth_config, enabled, available)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, true, $9)
        RETURNING id, name, description, domain, endpoint,
                  visibility, transport, config, oauth_config,
                  default_response_size_limit, owner_id,
                  enabled, available, created_at, updated_at
        "#,
    )
    .bind(name)
    .bind(description)
    .bind(domain)
    .bind(endpoint)
    .bind(visibility)
    .bind(transport)
    .bind(config)
    .bind(oauth_config)
    .bind(available)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Update a server's fields. Only non-None fields are updated.
pub async fn update_server(
    pool: &PgPool,
    server_id: &str,
    name: Option<&str>,
    description: Option<&str>,
    domain: Option<&str>,
    endpoint: Option<&str>,
    config: Option<&serde_json::Value>,
    enabled: Option<bool>,
    visibility: Option<&VisibilityTier>,
    available: Option<bool>,
) -> Result<McpServerRow, McpError> {
    // Build dynamic update using COALESCE pattern
    let row = sqlx::query_as::<_, McpServerRow>(
        r#"
        UPDATE mcp_servers SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            domain = COALESCE($4, domain),
            endpoint = COALESCE($5, endpoint),
            config = COALESCE($6, config),
            enabled = COALESCE($7, enabled),
            visibility = COALESCE($8, visibility),
            available = COALESCE($9, available),
            updated_at = now()
        WHERE id = $1::uuid
        RETURNING id, name, description, domain, endpoint,
                  visibility, transport, config, oauth_config,
                  default_response_size_limit, owner_id,
                  enabled, available, created_at, updated_at
        "#,
    )
    .bind(server_id)
    .bind(name)
    .bind(description)
    .bind(domain)
    .bind(endpoint)
    .bind(config)
    .bind(enabled)
    .bind(visibility)
    .bind(available)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| McpError::NotFound(format!("Server {server_id} not found")))?;
    Ok(row)
}

/// Delete a server by ID.
pub async fn delete_server(pool: &PgPool, server_id: &str) -> Result<bool, McpError> {
    let result = sqlx::query("DELETE FROM mcp_servers WHERE id = $1::uuid")
        .bind(server_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// =============================================================================
// Preference queries
// =============================================================================

/// Get user preferences for all servers.
pub async fn get_user_preferences(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<UserMcpPreferenceRow>, McpError> {
    let rows = sqlx::query_as::<_, UserMcpPreferenceRow>(
        "SELECT user_id, server_id, enabled, updated_at FROM user_mcp_preferences WHERE user_id = $1::uuid",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Upsert a user preference (enable/disable a server for a user).
pub async fn set_user_preference(
    pool: &PgPool,
    user_id: &str,
    server_id: &str,
    enabled: bool,
) -> Result<(), McpError> {
    sqlx::query(
        r#"
        INSERT INTO user_mcp_preferences (user_id, server_id, enabled, updated_at)
        VALUES ($1::uuid, $2::uuid, $3, now())
        ON CONFLICT (user_id, server_id)
        DO UPDATE SET enabled = EXCLUDED.enabled, updated_at = now()
        "#,
    )
    .bind(user_id)
    .bind(server_id)
    .bind(enabled)
    .execute(pool)
    .await?;
    Ok(())
}

// =============================================================================
// Tool queries
// =============================================================================

/// Get tools for a server.
pub async fn list_server_tools(
    pool: &PgPool,
    server_id: &str,
) -> Result<Vec<McpServerToolRow>, McpError> {
    let rows = sqlx::query_as::<_, McpServerToolRow>(
        r#"
        SELECT id, server_id, name, description, manifest, response_size_limit, created_at
        FROM mcp_server_tools
        WHERE server_id = $1::uuid
        ORDER BY name
        "#,
    )
    .bind(server_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Count tools for a server.
pub async fn get_tool_count(pool: &PgPool, server_id: &str) -> Result<i64, McpError> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM mcp_server_tools WHERE server_id = $1::uuid",
    )
    .bind(server_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Replace all tools for a server (delete existing + insert new).
pub async fn replace_server_tools(
    pool: &PgPool,
    server_id: &str,
    tools: &[McpToolSummary],
) -> Result<(), McpError> {
    // Delete existing
    sqlx::query("DELETE FROM mcp_server_tools WHERE server_id = $1::uuid")
        .bind(server_id)
        .execute(pool)
        .await?;

    // Insert new
    for tool in tools {
        let manifest = serde_json::json!({
            "name": tool.name,
            "description": tool.description,
        });
        sqlx::query(
            r#"
            INSERT INTO mcp_server_tools (server_id, name, description, manifest)
            VALUES ($1::uuid, $2, $3, $4)
            "#,
        )
        .bind(server_id)
        .bind(&tool.name)
        .bind(&tool.description)
        .bind(&manifest)
        .execute(pool)
        .await?;
    }
    Ok(())
}

// =============================================================================
// Preference count (admin)
// =============================================================================

/// Count users who have enabled a specific server.
pub async fn get_user_preference_count(pool: &PgPool, server_id: &str) -> Result<i64, McpError> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM user_mcp_preferences WHERE server_id = $1::uuid AND enabled = true",
    )
    .bind(server_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

// =============================================================================
// OAuth token queries
// =============================================================================

/// Check if a user has a valid (non-expired) OAuth token for a server.
pub async fn has_valid_oauth_token(
    pool: &PgPool,
    user_id: &str,
    server_id: &str,
) -> Result<bool, McpError> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM mcp_oauth_tokens
            WHERE user_id = $1::uuid AND server_id = $2::uuid AND expires_at > now()
        )
        "#,
    )
    .bind(user_id)
    .bind(server_id)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

// =============================================================================
// Secret queries
// =============================================================================

/// Store or update an encrypted API key for a server.
pub async fn store_api_key(
    pool: &PgPool,
    server_id: &str,
    api_key_encrypted: &str,
    encryption_key_id: &str,
) -> Result<(), McpError> {
    sqlx::query(
        r#"
        INSERT INTO mcp_server_secrets (server_id, api_key_encrypted, encryption_key_id)
        VALUES ($1::uuid, $2, $3)
        ON CONFLICT (server_id)
        DO UPDATE SET api_key_encrypted = EXCLUDED.api_key_encrypted,
                      encryption_key_id = EXCLUDED.encryption_key_id,
                      updated_at = now()
        "#,
    )
    .bind(server_id)
    .bind(api_key_encrypted)
    .bind(encryption_key_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Get the encrypted API key for a server.
pub async fn get_api_key_encrypted(
    pool: &PgPool,
    server_id: &str,
) -> Result<Option<String>, McpError> {
    let row = sqlx::query_scalar::<_, Option<String>>(
        "SELECT api_key_encrypted FROM mcp_server_secrets WHERE server_id = $1::uuid",
    )
    .bind(server_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.flatten())
}

// =============================================================================
// Audit queries
// =============================================================================

/// Insert an audit log entry.
pub async fn insert_audit_log(
    pool: &PgPool,
    actor_id: &str,
    server_id: Option<&str>,
    server_name: &str,
    action: &str,
    details: Option<&serde_json::Value>,
) -> Result<(), McpError> {
    sqlx::query(
        r#"
        INSERT INTO mcp_config_audit (actor_id, server_id, server_name, action, details)
        VALUES ($1::uuid, $2::uuid, $3, $4, $5)
        "#,
    )
    .bind(actor_id)
    .bind(server_id)
    .bind(server_name)
    .bind(action)
    .bind(details)
    .execute(pool)
    .await?;
    Ok(())
}

/// Extract auth_type from a server's config JSONB.
pub fn extract_auth_type(config: &Option<serde_json::Value>) -> AuthType {
    config
        .as_ref()
        .and_then(|c| c.get("authType"))
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "api-key" => AuthType::ApiKey,
            "oauth" => AuthType::OAuth,
            _ => AuthType::None,
        })
        .unwrap_or(AuthType::None)
}
