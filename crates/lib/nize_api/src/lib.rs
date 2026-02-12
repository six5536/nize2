//! # nize_api
//!
//! HTTP API library for Nize.

pub mod config;
pub mod error;
pub mod generated;
pub mod handlers;

use axum::Router;
use axum::routing::get;
use sqlx::PgPool;

use crate::config::ApiConfig;
use crate::generated::routes;
use crate::handlers::hello;

/// Shared application state passed to all handlers.
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool.
    pub pool: PgPool,
    /// API configuration.
    pub config: ApiConfig,
}

/// Builds the Axum router with all routes and shared state.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route(routes::GET_API_HELLO, get(hello::hello_world))
        .with_state(state)
}
