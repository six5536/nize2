// @zen-component: MCP-Auth
//
//! MCP bearer token authentication middleware.
//!
//! Validates `Authorization: Bearer <token>` headers against the `mcp_tokens` table.

use axum::{
    extract::State,
    http::{Request, StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::Response,
};
use sqlx::PgPool;
use tracing::debug;

/// Axum middleware: validates MCP bearer tokens.
///
/// Extracts the `Authorization: Bearer <token>` header, hashes the token with
/// SHA-256, and looks it up in the `mcp_tokens` table. Returns 401 if the
/// token is missing, malformed, expired, or revoked.
pub async fn mcp_auth_middleware(
    State(pool): State<PgPool>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request.headers().get(AUTHORIZATION);

    let token = match auth_header {
        Some(header) => {
            let header_str = header.to_str().unwrap_or("");
            match header_str.strip_prefix("Bearer ") {
                Some(t) => t.to_string(),
                None => {
                    debug!("MCP auth: missing Bearer prefix");
                    return Err(StatusCode::UNAUTHORIZED);
                }
            }
        }
        None => {
            debug!("MCP auth: no Authorization header");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    match nize_core::auth::mcp_tokens::validate_mcp_token(&pool, &token).await {
        Ok(Some(_user)) => Ok(next.run(request).await),
        Ok(None) => {
            debug!("MCP auth: token not found or revoked/expired");
            Err(StatusCode::UNAUTHORIZED)
        }
        Err(e) => {
            debug!("MCP auth: database error: {e}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
