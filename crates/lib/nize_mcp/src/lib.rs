//! # nize_mcp
//!
//! MCP (Model Context Protocol) server for Nize.
//!
//! Provides a Streamable HTTP MCP server with bearer token authentication.
//! The server is built as a library crate; `nize_desktop_server` wires it up on
//! a dedicated port.

pub mod auth;
pub mod server;
pub mod tools;

use std::sync::Arc;

use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

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
/// * `ct` — cancellation token for graceful shutdown of SSE streams.
pub fn mcp_router(pool: PgPool, ct: CancellationToken) -> axum::Router {
    let pool_for_service = pool.clone();

    let service: StreamableHttpService<server::NizeMcpServer, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(server::NizeMcpServer::new(pool_for_service.clone())),
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
