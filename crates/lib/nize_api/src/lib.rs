//! # nize_api
//!
//! HTTP API library for Nize.

pub mod config;
pub mod error;
pub mod generated;
pub mod handlers;
pub mod middleware;
pub mod services;

use std::sync::Arc;

use axum::Router;
use axum::http::Method;
use axum::http::header;
use axum::routing::{delete, get, patch, post};
use sqlx::PgPool;
use tokio::sync::RwLock;
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer};

use crate::config::ApiConfig;
use crate::generated::routes;
use crate::handlers::config as config_handlers;
use crate::handlers::{auth, hello, mcp_tokens};

use nize_core::config::cache::ConfigCache;

/// Shared application state passed to all handlers.
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool.
    pub pool: PgPool,
    /// API configuration.
    pub config: ApiConfig,
    /// In-memory config cache.
    pub config_cache: Arc<RwLock<ConfigCache>>,
}

/// Run embedded database migrations.
///
/// Delegates to `nize_core::migrate::migrate()` which owns the migration files.
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    nize_core::migrate::migrate(pool).await
}

/// Builds the Axum router with all routes and shared state.
pub fn router(state: AppState) -> Router {
    // CORS: allow credentials (cookies) with permissive origins.
    // In production, restrict allow_origin to specific domains.
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(AllowHeaders::list([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::ACCEPT,
            header::COOKIE,
        ]))
        .allow_credentials(true);

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
        .route(
            routes::GET_CONFIG_USER,
            get(config_handlers::user_config_list_handler),
        )
        .route(
            routes::PATCH_CONFIG_USER_KEY,
            patch(config_handlers::user_config_update_handler),
        )
        .route(
            routes::DELETE_CONFIG_USER_KEY,
            delete(config_handlers::user_config_reset_handler),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::auth::require_auth,
        ));

    // Admin routes (require admin role)
    let admin = Router::new()
        .route(
            routes::GET_ADMIN_CONFIG,
            get(config_handlers::admin_config_list_handler),
        )
        .route(
            routes::PATCH_ADMIN_CONFIG_SCOPE_KEY,
            patch(config_handlers::admin_config_update_handler),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::auth::require_admin,
        ));

    Router::new()
        .merge(public)
        .merge(protected)
        .merge(admin)
        .layer(cors)
        .with_state(state)
}
