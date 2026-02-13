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
use axum::routing::{delete, get, post};
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};

use crate::config::ApiConfig;
use crate::generated::routes;
use crate::handlers::{auth, hello, mcp_tokens};

/// Shared application state passed to all handlers.
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool.
    pub pool: PgPool,
    /// API configuration.
    pub config: ApiConfig,
}

/// Run embedded database migrations.
///
/// Delegates to `nize_core::migrate::migrate()` which owns the migration files.
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    nize_core::migrate::migrate(pool).await
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

    // Protected routes (require auth)
    let protected = Router::new()
        .route(
            routes::POST_AUTH_MCP_TOKENS,
            post(mcp_tokens::create_mcp_token_handler),
        )
        .route(
            routes::GET_AUTH_MCP_TOKENS,
            get(mcp_tokens::list_mcp_tokens_handler),
        )
        .route(
            routes::DELETE_AUTH_MCP_TOKENS_ID,
            delete(mcp_tokens::revoke_mcp_token_handler),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::auth::require_auth,
        ));

    Router::new()
        .merge(public)
        .merge(protected)
        .layer(cors)
        .with_state(state)
}
