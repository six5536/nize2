//! # nize_api
//!
//! HTTP API library for Nize.

pub mod config;
pub mod error;
pub mod generated;
pub mod handlers;
pub mod middleware;
pub mod services;

use axum::Router;
use axum::routing::{get, post};
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};

use crate::config::ApiConfig;
use crate::generated::routes;
use crate::handlers::{auth, hello};

/// Shared application state passed to all handlers.
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool.
    pub pool: PgPool,
    /// API configuration.
    pub config: ApiConfig,
}

/// Run embedded database migrations.
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

/// Builds the Axum router with all routes and shared state.
pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public routes (no auth required)
    let public = Router::new()
        .route(routes::GET_API_HELLO, get(hello::hello_world))
        .route(routes::POST_AUTH_LOGIN, post(auth::login_handler))
        .route(routes::POST_AUTH_REGISTER, post(auth::register_handler))
        .route(routes::POST_AUTH_REFRESH, post(auth::refresh_handler))
        .route(routes::POST_AUTH_LOGOUT, post(auth::logout_handler))
        .route(routes::GET_AUTH_STATUS, get(auth::auth_status_handler));

    Router::new().merge(public).layer(cors).with_state(state)
}
