//! API server configuration.

use crate::services::auth::resolve_jwt_secret;

/// Configuration for the API server.
#[derive(Clone, Debug)]
pub struct ApiConfig {
    /// Address to bind the HTTP listener (e.g. "127.0.0.1:3100").
    pub bind_addr: String,
    /// PostgreSQL connection URL.
    pub pg_connection_url: String,
    /// JWT signing secret.
    pub jwt_secret: String,
    /// Encryption key for MCP server secrets (API keys, OAuth secrets).
    pub mcp_encryption_key: String,
}

impl ApiConfig {
    /// Reads configuration from environment variables with sensible defaults.
    ///
    /// | Variable           | Default                                     |
    /// |--------------------|---------------------------------------------|
    /// | `BIND_ADDR`        | `127.0.0.1:3100`                            |
    /// | `DATABASE_URL`     | `postgres://localhost:5432/nize`             |
    /// | `JWT_SECRET` / `AUTH_SECRET` | generated & persisted to file        |
    pub fn from_env() -> Self {
        Self {
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3100".into()),
            pg_connection_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://localhost:5432/nize".into()),
            jwt_secret: resolve_jwt_secret(),
            mcp_encryption_key: std::env::var("MCP_ENCRYPTION_KEY")
                .unwrap_or_else(|_| "nize-mcp-default-dev-key-change-in-production".into()),
        }
    }
}
