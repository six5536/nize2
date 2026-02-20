//! MCP server registry database queries.
//!
//! Raw SQLx queries for CRUD operations on MCP tables.

use sqlx::PgPool;

use super::McpError;
use crate::models::mcp::{
    AuthType, McpOauthTokenRow, McpServerRow, McpServerToolRow, McpToolSummary, ServerConfig,
    TransportType, UserMcpPreferenceRow, VisibilityTier,
};
use crate::uuid::uuidv7;

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
#[allow(clippy::too_many_arguments)]
pub async fn insert_user_server(
    pool: &PgPool,
    user_id: &str,
    name: &str,
    description: &str,
    domain: &str,
    config: &ServerConfig,
    oauth_config: Option<&serde_json::Value>,
    available: bool,
) -> Result<McpServerRow, McpError> {
    let config_json = serde_json::to_value(config)
        .map_err(|e| McpError::Validation(format!("Failed to serialize config: {e}")))?;

    let row = sqlx::query_as::<_, McpServerRow>(
        r#"
        INSERT INTO mcp_servers (id, name, description, domain, endpoint, visibility, transport, config, oauth_config, owner_id, enabled, available)
        VALUES ($1, $2, $3, $4, $5, 'user', $6, $7, $8, $9::uuid, true, $10)
        RETURNING id, name, description, domain, endpoint,
                  visibility, transport, config, oauth_config,
                  default_response_size_limit, owner_id,
                  enabled, available, created_at, updated_at
        "#,
    )
    .bind(uuidv7())
    .bind(name)
    .bind(description)
    .bind(domain)
    .bind(config.endpoint())
    .bind(config.transport_type())
    .bind(&config_json)
    .bind(oauth_config)
    .bind(user_id)
    .bind(available)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Insert a built-in server (admin).
#[allow(clippy::too_many_arguments)]
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
        INSERT INTO mcp_servers (id, name, description, domain, endpoint, visibility, transport, config, oauth_config, enabled, available)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, true, $10)
        RETURNING id, name, description, domain, endpoint,
                  visibility, transport, config, oauth_config,
                  default_response_size_limit, owner_id,
                  enabled, available, created_at, updated_at
        "#,
    )
    .bind(uuidv7())
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
#[allow(clippy::too_many_arguments)]
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
    oauth_config: Option<&serde_json::Value>,
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
            oauth_config = COALESCE($10, oauth_config),
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
    .bind(oauth_config)
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
            INSERT INTO mcp_server_tools (id, server_id, name, description, manifest)
            VALUES ($1, $2::uuid, $3, $4, $5)
            "#,
        )
        .bind(uuidv7())
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

/// Store or update OAuth tokens for a user+server pair.
#[allow(clippy::too_many_arguments)]
pub async fn store_oauth_token(
    pool: &PgPool,
    user_id: &str,
    server_id: &str,
    id_token_encrypted: Option<&str>,
    access_token_encrypted: &str,
    refresh_token_encrypted: Option<&str>,
    expires_at: chrono::DateTime<chrono::Utc>,
    scopes: &[String],
) -> Result<(), McpError> {
    sqlx::query(
        r#"
        INSERT INTO mcp_oauth_tokens
            (user_id, server_id, id_token_encrypted, access_token_encrypted,
             refresh_token_encrypted, expires_at, scopes)
        VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7)
        ON CONFLICT (user_id, server_id)
        DO UPDATE SET id_token_encrypted = EXCLUDED.id_token_encrypted,
                      access_token_encrypted = EXCLUDED.access_token_encrypted,
                      refresh_token_encrypted = COALESCE(EXCLUDED.refresh_token_encrypted, mcp_oauth_tokens.refresh_token_encrypted),
                      expires_at = EXCLUDED.expires_at,
                      scopes = EXCLUDED.scopes,
                      updated_at = now()
        "#,
    )
    .bind(user_id)
    .bind(server_id)
    .bind(id_token_encrypted)
    .bind(access_token_encrypted)
    .bind(refresh_token_encrypted)
    .bind(expires_at)
    .bind(scopes)
    .execute(pool)
    .await?;
    Ok(())
}

/// Get OAuth tokens for a user+server pair (regardless of expiry).
pub async fn get_oauth_token(
    pool: &PgPool,
    user_id: &str,
    server_id: &str,
) -> Result<Option<McpOauthTokenRow>, McpError> {
    let row = sqlx::query_as::<_, McpOauthTokenRow>(
        r#"
        SELECT user_id, server_id, id_token_encrypted, access_token_encrypted,
               refresh_token_encrypted, expires_at, scopes, created_at, updated_at
        FROM mcp_oauth_tokens
        WHERE user_id = $1::uuid AND server_id = $2::uuid
        "#,
    )
    .bind(user_id)
    .bind(server_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Delete OAuth tokens for a user+server pair.
pub async fn delete_oauth_token(
    pool: &PgPool,
    user_id: &str,
    server_id: &str,
) -> Result<(), McpError> {
    sqlx::query("DELETE FROM mcp_oauth_tokens WHERE user_id = $1::uuid AND server_id = $2::uuid")
        .bind(user_id)
        .bind(server_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete all OAuth tokens for a server (all users).
pub async fn delete_all_oauth_tokens_for_server(
    pool: &PgPool,
    server_id: &str,
) -> Result<u64, McpError> {
    let result = sqlx::query("DELETE FROM mcp_oauth_tokens WHERE server_id = $1::uuid")
        .bind(server_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
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
        INSERT INTO mcp_server_secrets (id, server_id, api_key_encrypted, encryption_key_id)
        VALUES ($1, $2::uuid, $3, $4)
        ON CONFLICT (server_id)
        DO UPDATE SET api_key_encrypted = EXCLUDED.api_key_encrypted,
                      encryption_key_id = EXCLUDED.encryption_key_id,
                      updated_at = now()
        "#,
    )
    .bind(uuidv7())
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

/// Store or update an encrypted OAuth client secret for a server.
pub async fn store_oauth_client_secret(
    pool: &PgPool,
    server_id: &str,
    secret_encrypted: &str,
    encryption_key_id: &str,
) -> Result<(), McpError> {
    sqlx::query(
        r#"
        INSERT INTO mcp_server_secrets (id, server_id, oauth_client_secret_encrypted, encryption_key_id)
        VALUES ($1, $2::uuid, $3, $4)
        ON CONFLICT (server_id)
        DO UPDATE SET oauth_client_secret_encrypted = EXCLUDED.oauth_client_secret_encrypted,
                      encryption_key_id = EXCLUDED.encryption_key_id,
                      updated_at = now()
        "#,
    )
    .bind(uuidv7())
    .bind(server_id)
    .bind(secret_encrypted)
    .bind(encryption_key_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Get the encrypted OAuth client secret for a server.
pub async fn get_oauth_client_secret_encrypted(
    pool: &PgPool,
    server_id: &str,
) -> Result<Option<String>, McpError> {
    let row = sqlx::query_scalar::<_, Option<String>>(
        "SELECT oauth_client_secret_encrypted FROM mcp_server_secrets WHERE server_id = $1::uuid",
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
        INSERT INTO mcp_config_audit (id, actor_id, server_id, server_name, action, details)
        VALUES ($1, $2::uuid, $3::uuid, $4, $5, $6)
        "#,
    )
    .bind(uuidv7())
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
    fn parse_auth_type(value: &str) -> AuthType {
        match value {
            "api-key" => AuthType::ApiKey,
            "oauth" => AuthType::OAuth,
            _ => AuthType::None,
        }
    }

    let Some(raw) = config.as_ref() else {
        return AuthType::None;
    };

    if let Ok(parsed) = serde_json::from_value::<ServerConfig>(raw.clone()) {
        return match parsed {
            ServerConfig::Http(http) => parse_auth_type(&http.auth_type),
            ServerConfig::Sse(sse) => parse_auth_type(&sse.auth_type),
            _ => AuthType::None,
        };
    }

    raw.get("authType")
        .or_else(|| raw.get("auth_type"))
        .or_else(|| raw.get("config").and_then(|c| c.get("authType")))
        .or_else(|| raw.get("config").and_then(|c| c.get("auth_type")))
        .and_then(|v| v.as_str())
        .map(parse_auth_type)
        .unwrap_or(AuthType::None)
}

// =============================================================================
// Discovery queries (tool domains, manifests)
// =============================================================================

/// A tool domain with its tool count.
#[derive(Debug, Clone)]
pub struct ToolDomainRow {
    pub domain: String,
    pub tool_count: i64,
}

/// List distinct tool domains visible to a user, with tool counts.
///
/// Filters by servers the user has access to: globally visible servers
/// (unless explicitly disabled) or explicitly enabled servers.
pub async fn list_tool_domains(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<ToolDomainRow>, McpError> {
    let rows = sqlx::query_as::<_, (String, i64)>(
        r#"
        SELECT s.domain, COUNT(*) AS tool_count
        FROM mcp_server_tools t
        JOIN mcp_servers s ON s.id = t.server_id
        WHERE s.enabled = true
          AND (
            (s.visibility = 'visible' AND NOT EXISTS (
              SELECT 1 FROM user_mcp_preferences p
              WHERE p.user_id = $1::uuid AND p.server_id = s.id AND p.enabled = false
            ))
            OR EXISTS (
              SELECT 1 FROM user_mcp_preferences p
              WHERE p.user_id = $1::uuid AND p.server_id = s.id AND p.enabled = true
            )
          )
        GROUP BY s.domain
        ORDER BY s.domain
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(domain, tool_count)| ToolDomainRow { domain, tool_count })
        .collect())
}

/// A tool row returned by domain browsing.
#[derive(Debug, Clone)]
pub struct BrowseToolRow {
    pub tool_id: sqlx::types::Uuid,
    pub tool_name: String,
    pub tool_description: String,
    pub domain: String,
    pub server_id: sqlx::types::Uuid,
    pub server_name: String,
}

/// Browse all tools in a domain, filtered by user-enabled servers.
pub async fn browse_tool_domain(
    pool: &PgPool,
    user_id: &str,
    domain: &str,
) -> Result<Vec<BrowseToolRow>, McpError> {
    let rows = sqlx::query_as::<
        _,
        (
            sqlx::types::Uuid,
            String,
            String,
            String,
            sqlx::types::Uuid,
            String,
        ),
    >(
        r#"
        SELECT t.id, t.name, t.description, s.domain, s.id, s.name
        FROM mcp_server_tools t
        JOIN mcp_servers s ON s.id = t.server_id
        WHERE s.enabled = true
          AND s.domain = $2
          AND (
            (s.visibility = 'visible' AND NOT EXISTS (
              SELECT 1 FROM user_mcp_preferences p
              WHERE p.user_id = $1::uuid AND p.server_id = s.id AND p.enabled = false
            ))
            OR EXISTS (
              SELECT 1 FROM user_mcp_preferences p
              WHERE p.user_id = $1::uuid AND p.server_id = s.id AND p.enabled = true
            )
          )
        ORDER BY t.name
        "#,
    )
    .bind(user_id)
    .bind(domain)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(tool_id, tool_name, tool_description, domain, server_id, server_name)| {
                BrowseToolRow {
                    tool_id,
                    tool_name,
                    tool_description,
                    domain,
                    server_id,
                    server_name,
                }
            },
        )
        .collect())
}

/// Get a tool manifest by tool ID, verifying user access.
///
/// Returns `None` if the tool doesn't exist or the user doesn't have access
/// to the server hosting it.
pub async fn get_tool_manifest(
    pool: &PgPool,
    user_id: &str,
    tool_id: &str,
) -> Result<Option<McpServerToolRow>, McpError> {
    let row = sqlx::query_as::<_, McpServerToolRow>(
        r#"
        SELECT t.id, t.server_id, t.name, t.description, t.manifest,
               t.response_size_limit, t.created_at
        FROM mcp_server_tools t
        JOIN mcp_servers s ON s.id = t.server_id
        WHERE t.id = $2::uuid
          AND s.enabled = true
          AND (
            (s.visibility = 'visible' AND NOT EXISTS (
              SELECT 1 FROM user_mcp_preferences p
              WHERE p.user_id = $1::uuid AND p.server_id = s.id AND p.enabled = false
            ))
            OR EXISTS (
              SELECT 1 FROM user_mcp_preferences p
              WHERE p.user_id = $1::uuid AND p.server_id = s.id AND p.enabled = true
            )
          )
        "#,
    )
    .bind(user_id)
    .bind(tool_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Check if a user has access to a specific server.
///
/// A user has access if:
/// - The server is visible and user hasn't explicitly disabled it, OR
/// - The user has explicitly enabled it (including user-owned servers).
pub async fn user_has_server_access(
    pool: &PgPool,
    user_id: &str,
    server_id: &str,
) -> Result<bool, McpError> {
    let has_access = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM mcp_servers s
            WHERE s.id = $2::uuid
              AND s.enabled = true
              AND (
                (s.visibility = 'visible' AND NOT EXISTS (
                  SELECT 1 FROM user_mcp_preferences p
                  WHERE p.user_id = $1::uuid AND p.server_id = s.id AND p.enabled = false
                ))
                OR EXISTS (
                  SELECT 1 FROM user_mcp_preferences p
                  WHERE p.user_id = $1::uuid AND p.server_id = s.id AND p.enabled = true
                )
              )
        )
        "#,
    )
    .bind(user_id)
    .bind(server_id)
    .fetch_one(pool)
    .await?;
    Ok(has_access)
}
