//! MCP server registry domain models.
//!
//! These are internal domain models for MCP server configuration,
//! aligned with the reference project's `packages/agent/src/mcp/types.ts`.

use serde::{Deserialize, Serialize};

// =============================================================================
// Enums
// =============================================================================

/// Server visibility tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "visibility_tier", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum VisibilityTier {
    Hidden,
    Visible,
    User,
}

/// Transport type for MCP servers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "transport_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    Stdio,
    Http,
}

/// Authentication type for MCP servers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "auth_type")]
#[serde(rename_all = "kebab-case")]
pub enum AuthType {
    #[sqlx(rename = "none")]
    #[serde(rename = "none")]
    None,
    #[sqlx(rename = "api-key")]
    #[serde(rename = "api-key")]
    ApiKey,
    #[sqlx(rename = "oauth")]
    #[serde(rename = "oauth")]
    OAuth,
}

/// Computed server status for display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    Enabled,
    Disabled,
    Unavailable,
    Unauthorized,
}

// =============================================================================
// DB row structs
// =============================================================================

/// Database row for `mcp_servers`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct McpServerRow {
    pub id: sqlx::types::Uuid,
    pub name: String,
    pub description: String,
    pub domain: String,
    pub endpoint: String,
    pub visibility: VisibilityTier,
    pub transport: TransportType,
    pub config: Option<serde_json::Value>,
    pub oauth_config: Option<serde_json::Value>,
    pub default_response_size_limit: Option<i32>,
    pub owner_id: Option<sqlx::types::Uuid>,
    pub enabled: bool,
    pub available: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Database row for `mcp_server_tools`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct McpServerToolRow {
    pub id: sqlx::types::Uuid,
    pub server_id: sqlx::types::Uuid,
    pub name: String,
    pub description: String,
    pub manifest: serde_json::Value,
    pub response_size_limit: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Database row for `user_mcp_preferences`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserMcpPreferenceRow {
    pub user_id: sqlx::types::Uuid,
    pub server_id: sqlx::types::Uuid,
    pub enabled: bool,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Database row for `mcp_server_secrets`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct McpServerSecretRow {
    pub id: sqlx::types::Uuid,
    pub server_id: sqlx::types::Uuid,
    pub api_key_encrypted: Option<String>,
    pub oauth_client_secret_encrypted: Option<String>,
    pub encryption_key_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Database row for `mcp_oauth_tokens`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct McpOauthTokenRow {
    pub user_id: sqlx::types::Uuid,
    pub server_id: sqlx::types::Uuid,
    pub id_token_encrypted: Option<String>,
    pub access_token_encrypted: String,
    pub refresh_token_encrypted: Option<String>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub scopes: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// =============================================================================
// View structs (API responses)
// =============================================================================

/// User view of a server (used in `/settings/tools`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserServerView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    pub visibility: VisibilityTier,
    pub status: ServerStatus,
    pub tool_count: i64,
    pub is_owned: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Admin view of a server (used in `/admin/tools`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminServerView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    pub visibility: VisibilityTier,
    pub status: ServerStatus,
    pub tool_count: i64,
    pub is_owned: bool,
    pub transport: TransportType,
    pub auth_type: AuthType,
    pub owner_id: Option<String>,
    pub user_preference_count: i64,
    pub enabled: bool,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_config: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

/// Tool summary returned from server tools endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolSummary {
    pub name: String,
    pub description: String,
}

// =============================================================================
// Config types (stored in JSONB)
// =============================================================================

/// Stdio-based MCP server configuration.
/// Admin-only: spawns local subprocess.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StdioServerConfig {
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<std::collections::HashMap<String, String>>,
}

/// HTTP-based MCP server configuration.
/// Supports no auth, API key, or OAuth authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpServerConfig {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
    pub auth_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_header: Option<String>,
}

/// Discriminated union for MCP server transport configuration.
/// Uses `transport` as the tag field, following idiomatic MCP conventions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "transport")]
pub enum ServerConfig {
    #[serde(rename = "stdio")]
    Stdio(StdioServerConfig),
    #[serde(rename = "http")]
    Http(HttpServerConfig),
}

impl ServerConfig {
    /// Get the endpoint string (URL for HTTP, command for stdio).
    pub fn endpoint(&self) -> &str {
        match self {
            Self::Http(c) => &c.url,
            Self::Stdio(c) => &c.command,
        }
    }

    /// Get the transport type.
    pub fn transport_type(&self) -> TransportType {
        match self {
            Self::Http(_) => TransportType::Http,
            Self::Stdio(_) => TransportType::Stdio,
        }
    }
}

/// OAuth client configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfig {
    pub client_id: String,
    pub authorization_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
}

// =============================================================================
// Result types
// =============================================================================

/// Result from deleting a built-in server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub deleted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affected_users: Option<i64>,
}

/// Result from testing a connection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_details: Option<String>,
    /// Indicates the server requires OAuth authorization before connection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_required: Option<bool>,
    /// Tools discovered during connection test (not serialized in response).
    #[serde(skip)]
    pub tools: Vec<McpToolSummary>,
}
