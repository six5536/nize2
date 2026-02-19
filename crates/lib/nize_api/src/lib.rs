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
use axum::routing::{delete, get, patch, post, put};
use sqlx::PgPool;
use tokio::sync::RwLock;
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer};

use crate::config::ApiConfig;
use crate::generated::routes;
use crate::handlers::config as config_handlers;
use crate::handlers::{
    admin_permissions, ai_proxy, auth, chat, conversations, embeddings, hello, ingest, mcp_config,
    mcp_tokens, oauth, permissions, trace,
};

use nize_core::config::cache::ConfigCache;

/// Path prefix under which all API routes are nested.
pub const API_PREFIX: &str = "/api";
use nize_core::mcp::oauth::OAuthStateStore;

/// Shared application state passed to all handlers.
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool.
    pub pool: PgPool,
    /// API configuration.
    pub config: ApiConfig,
    /// In-memory config cache.
    pub config_cache: Arc<RwLock<ConfigCache>>,
    /// In-memory OAuth PKCE state store.
    pub oauth_state: Arc<OAuthStateStore>,
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
        .route(routes::GET_HELLO, get(hello::hello_world))
        .route(routes::POST_AUTH_LOGIN, post(auth::login_handler))
        .route(routes::POST_AUTH_REGISTER, post(auth::register_handler))
        .route(routes::POST_AUTH_REFRESH, post(auth::refresh_handler))
        .route(routes::POST_AUTH_LOGOUT, post(auth::logout_handler))
        .route(routes::GET_AUTH_STATUS, get(auth::auth_status_handler))
        .route(
            routes::GET_AUTH_OAUTH_MCP_CALLBACK,
            get(oauth::oauth_callback_handler),
        )
        .route(
            routes::GET_PERMISSIONS_SHARED_TOKEN,
            get(permissions::access_shared_handler),
        );

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
        .route(routes::POST_AUTH_LOGOUT_ALL, post(auth::logout_all_handler))
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
        // Chat
        .route(routes::POST_CHAT, post(chat::chat_handler))
        // AI Proxy
        .route("/ai-proxy", post(ai_proxy::ai_proxy_handler))
        // Conversations
        .route(
            routes::GET_CONVERSATIONS,
            get(conversations::list_conversations_handler),
        )
        .route(
            routes::POST_CONVERSATIONS,
            post(conversations::create_conversation_handler),
        )
        .route(
            routes::GET_CONVERSATIONS_ID,
            get(conversations::get_conversation_handler),
        )
        .route(
            routes::PATCH_CONVERSATIONS_ID,
            patch(conversations::update_conversation_handler),
        )
        .route(
            routes::DELETE_CONVERSATIONS_ID,
            delete(conversations::delete_conversation_handler),
        )
        .route(
            routes::PUT_CONVERSATIONS_ID_MESSAGES,
            put(conversations::save_messages_handler),
        )
        // Ingest
        .route(routes::GET_INGEST, get(ingest::list_documents_handler))
        .route(routes::POST_INGEST, post(ingest::upload_handler))
        .route(routes::GET_INGEST_ID, get(ingest::get_document_handler))
        .route(
            routes::DELETE_INGEST_ID,
            delete(ingest::delete_document_handler),
        )
        // Permissions — grants
        .route(
            routes::POST_PERMISSIONS_RESOURCETYPE_RESOURCEID_GRANTS,
            post(permissions::create_grant_handler),
        )
        .route(
            routes::GET_PERMISSIONS_RESOURCETYPE_RESOURCEID_GRANTS,
            get(permissions::list_grants_handler),
        )
        .route(
            routes::DELETE_PERMISSIONS_GRANTS_GRANTID,
            delete(permissions::revoke_grant_handler),
        )
        // Permissions — links
        .route(
            routes::POST_PERMISSIONS_RESOURCETYPE_RESOURCEID_LINKS,
            post(permissions::create_link_handler),
        )
        .route(
            routes::GET_PERMISSIONS_RESOURCETYPE_RESOURCEID_LINKS,
            get(permissions::list_links_handler),
        )
        .route(
            routes::DELETE_PERMISSIONS_LINKS_LINKID,
            delete(permissions::revoke_link_handler),
        )
        // MCP servers (user)
        .route(
            routes::GET_MCP_SERVERS,
            get(mcp_config::list_servers_handler),
        )
        .route(
            routes::POST_MCP_SERVERS,
            post(mcp_config::add_server_handler),
        )
        .route(
            routes::PATCH_MCP_SERVERS_SERVERID,
            patch(mcp_config::update_server_handler),
        )
        .route(
            routes::DELETE_MCP_SERVERS_SERVERID,
            delete(mcp_config::delete_server_handler),
        )
        .route(
            routes::PATCH_MCP_SERVERS_SERVERID_PREFERENCE,
            patch(mcp_config::update_preference_handler),
        )
        .route(
            routes::GET_MCP_SERVERS_SERVERID_TOOLS,
            get(mcp_config::list_server_tools_handler),
        )
        .route(
            routes::GET_MCP_SERVERS_SERVERID_OAUTH_STATUS,
            get(mcp_config::oauth_status_handler),
        )
        .route(
            routes::POST_MCP_SERVERS_SERVERID_OAUTH_INITIATE,
            post(mcp_config::oauth_initiate_handler),
        )
        .route(
            routes::POST_MCP_SERVERS_SERVERID_OAUTH_REVOKE,
            post(mcp_config::oauth_revoke_handler),
        )
        .route(
            routes::POST_MCP_TEST_CONNECTION,
            post(mcp_config::test_connection_handler),
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
        // Admin permissions
        .route(
            routes::GET_ADMIN_PERMISSIONS_GRANTS,
            get(admin_permissions::list_all_grants_handler),
        )
        .route(
            routes::DELETE_ADMIN_PERMISSIONS_GRANTS_GRANTID,
            delete(admin_permissions::admin_revoke_grant_handler),
        )
        .route(
            routes::GET_ADMIN_PERMISSIONS_GROUPS,
            get(admin_permissions::list_all_groups_handler),
        )
        .route(
            routes::GET_ADMIN_PERMISSIONS_LINKS,
            get(admin_permissions::list_all_links_handler),
        )
        .route(
            routes::DELETE_ADMIN_PERMISSIONS_LINKS_LINKID,
            delete(admin_permissions::admin_revoke_link_handler),
        )
        .route(
            routes::PATCH_ADMIN_PERMISSIONS_USERS_USERID_ADMIN,
            patch(admin_permissions::set_admin_role_handler),
        )
        // Admin MCP servers
        .route(
            routes::GET_MCP_ADMIN_SERVERS,
            get(mcp_config::admin_list_servers_handler),
        )
        .route(
            routes::POST_MCP_ADMIN_SERVERS,
            post(mcp_config::admin_create_server_handler),
        )
        .route(
            routes::PATCH_MCP_ADMIN_SERVERS_SERVERID,
            patch(mcp_config::admin_update_server_handler),
        )
        .route(
            routes::DELETE_MCP_ADMIN_SERVERS_SERVERID,
            delete(mcp_config::admin_delete_server_handler),
        )
        // Admin embeddings
        .route(
            "/admin/embeddings/models",
            get(embeddings::list_models_handler),
        )
        .route("/admin/embeddings/search", post(embeddings::search_handler))
        .route(
            "/admin/embeddings/reindex",
            post(embeddings::reindex_handler),
        )
        // Dev trace
        .route(routes::GET_DEV_CHAT_TRACE, get(trace::chat_trace_handler))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::auth::require_admin,
        ));

    // All routes are nested under /api so they don't collide with
    // the Next.js frontend routes when served on the same origin.
    let api = Router::new().merge(public).merge(protected).merge(admin);

    Router::new()
        .nest(API_PREFIX, api)
        .layer(cors)
        .with_state(state)
}
