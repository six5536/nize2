// @awa-component: CFG-ConfigCache
//
//! Configuration module — cache, resolution, and validation.

pub mod cache;
pub mod queries;
pub mod resolver;
pub mod validation;

use thiserror::Error;

/// Configuration errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Config key not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Database error: {0}")]
    DbError(#[from] sqlx::Error),
}
