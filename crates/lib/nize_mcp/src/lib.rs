//! # nize_mcp
//!
//! MCP (Model Context Protocol) server for Nize.
//!
//! Provides a Streamable HTTP MCP server with bearer token authentication.
//! The server is built as a library crate; `nize_desktop_server` wires it up on
//! a dedicated port.

pub mod auth;
pub mod hooks;
pub mod server;
pub mod tools;

use std::sync::Arc;

use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use sqlx::PgPool;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use nize_core::config::cache::ConfigCache;
use nize_core::mcp::execution::ClientPool;

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Build an Axum router that serves the MCP Streamable HTTP endpoint at `/mcp`.
///
/// The router includes bearer token authentication middleware. Every request
/// must carry `Authorization: Bearer <token>` where the token has been created
/// via `POST /auth/mcp-tokens`.
///
/// # Arguments
///
/// * `pool` — shared database connection pool (same pool as the REST API).
/// * `config_cache` — shared config cache for embedding resolution.
/// * `ct` — cancellation token for graceful shutdown of SSE streams.
pub fn mcp_router(
    pool: PgPool,
    config_cache: Arc<RwLock<ConfigCache>>,
    ct: CancellationToken,
    encryption_key: String,
) -> axum::Router {
    mcp_router_with_manifest(pool, config_cache, ct, None, encryption_key)
}

/// Build an Axum router with an optional terminator manifest path.
///
/// When `manifest_path` is `Some`, stdio MCP server process PIDs are
/// appended to the manifest file for crash recovery by nize_terminator.
pub fn mcp_router_with_manifest(
    pool: PgPool,
    config_cache: Arc<RwLock<ConfigCache>>,
    ct: CancellationToken,
    manifest_path: Option<std::path::PathBuf>,
    encryption_key: String,
) -> axum::Router {
    let pool_for_service = pool.clone();

    let hook_pipeline = Arc::new(hooks::default_pipeline(pool.clone()));
    let client_pool = Arc::new(match manifest_path {
        Some(path) => ClientPool::with_manifest(path),
        None => ClientPool::new(),
    });

    // @awa-impl: PLAN-030 Phase 2.3 — spawn idle timeout reaper
    let _reaper = client_pool.spawn_reaper(client_pool.idle_timeout());

    let service: StreamableHttpService<server::NizeMcpServer, LocalSessionManager> =
        StreamableHttpService::new(
            move || {
                Ok(server::NizeMcpServer::new(
                    pool_for_service.clone(),
                    config_cache.clone(),
                    client_pool.clone(),
                    hook_pipeline.clone(),
                    encryption_key.clone(),
                ))
            },
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig {
                stateful_mode: true,
                cancellation_token: ct,
                ..Default::default()
            },
        );

    axum::Router::new()
        .nest_service("/mcp", service)
        .layer(axum::middleware::from_fn_with_state(
            pool,
            auth::mcp_auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }
}
