//! Application error types.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

use crate::generated::models::ErrorResponse;

/// Convenience alias for handler return types.
pub type AppResult<T> = Result<T, AppError>;

/// Application-level errors with HTTP status mapping.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Database unavailable: {0}")]
    DbUnavailable(String),

    #[error("Sidecar unavailable: {0}")]
    SidecarUnavailable(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Internal server error")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error, message) = match &self {
            AppError::Validation(m) => (StatusCode::BAD_REQUEST, "validation_error", m.as_str()),
            AppError::NotFound(m) => (StatusCode::NOT_FOUND, "not_found", m.as_str()),
            AppError::DbUnavailable(m) => {
                (StatusCode::SERVICE_UNAVAILABLE, "db_unavailable", m.as_str())
            }
            AppError::SidecarUnavailable(m) => {
                (StatusCode::SERVICE_UNAVAILABLE, "sidecar_unavailable", m.as_str())
            }
            AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, "unauthorized", m.as_str()),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, "forbidden", m.as_str()),
            AppError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Internal server error",
            ),
        };
        let body = Json(ErrorResponse {
            error: error.to_string(),
            message: message.to_string(),
        });
        (status, body).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound("row not found".into()),
            _ => AppError::Internal(e.to_string()),
        }
    }
}

impl From<nize_core::auth::AuthError> for AppError {
    fn from(e: nize_core::auth::AuthError) -> Self {
        match e {
            nize_core::auth::AuthError::CredentialError => {
                AppError::Unauthorized("Invalid credentials".into())
            }
            nize_core::auth::AuthError::TokenError(msg) => AppError::Unauthorized(msg),
            nize_core::auth::AuthError::ValidationError(msg) => AppError::Validation(msg),
            nize_core::auth::AuthError::DbError(e) => AppError::from(e),
            nize_core::auth::AuthError::Internal(msg) => AppError::Internal(msg),
        }
    }
}

impl From<nize_core::mcp::McpError> for AppError {
    fn from(e: nize_core::mcp::McpError) -> Self {
        match e {
            nize_core::mcp::McpError::NotFound(msg) => AppError::NotFound(msg),
            nize_core::mcp::McpError::Forbidden(msg) => AppError::Forbidden(msg),
            nize_core::mcp::McpError::Validation(msg) => AppError::Validation(msg),
            nize_core::mcp::McpError::ServerLimitExceeded(n) => {
                AppError::Validation(format!("Maximum of {n} user servers allowed"))
            }
            nize_core::mcp::McpError::DuplicateServer(name) => {
                AppError::Validation(format!("Server with name '{name}' already exists"))
            }
            nize_core::mcp::McpError::InvalidTransport(msg) => AppError::Validation(msg),
            nize_core::mcp::McpError::ConnectionFailed(msg) => AppError::Validation(msg),
            nize_core::mcp::McpError::EncryptionError(msg) => AppError::Internal(msg),
            nize_core::mcp::McpError::DbError(e) => AppError::from(e),
        }
    }
}
