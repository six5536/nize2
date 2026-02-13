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
