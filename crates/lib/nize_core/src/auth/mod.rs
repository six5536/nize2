//! Authentication and authorization logic.
//!
//! Provides password hashing, JWT management, and database queries
//! that can be shared across `nize_api` and `nize_mcp`.

pub mod jwt;
pub mod mcp_tokens;
pub mod password;
pub mod queries;

use thiserror::Error;

/// Authentication errors.
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    CredentialError,

    #[error("Token error: {0}")]
    TokenError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Database error: {0}")]
    DbError(#[from] sqlx::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}
