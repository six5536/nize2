//! MCP server registry logic.
//!
//! Provides database queries, secret encryption, and shared business logic
//! for MCP server configuration.

pub mod queries;
pub mod secrets;

use thiserror::Error;

/// MCP configuration errors.
#[derive(Debug, Error)]
pub enum McpError {
    #[error("Server not found: {0}")]
    NotFound(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Server limit exceeded: maximum {0} user servers allowed")]
    ServerLimitExceeded(usize),

    #[error("Duplicate server name: {0}")]
    DuplicateServer(String),

    #[error("Invalid transport: {0}")]
    InvalidTransport(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Database error: {0}")]
    DbError(#[from] sqlx::Error),
}
