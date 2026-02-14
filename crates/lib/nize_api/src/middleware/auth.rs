// @zen-component: AUTH-AccessControl
//
//! Authentication middleware — dual auth: cookie first, Bearer fallback.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use axum::http::header::AUTHORIZATION;
use axum_extra::extract::CookieJar;

use crate::AppState;
use crate::error::AppError;
use crate::services::auth::{TokenClaims, verify_access_token};
use crate::services::cookies::ACCESS_COOKIE;

/// Key used to store `TokenClaims` in request extensions.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser(pub TokenClaims);

// @zen-impl: AUTH-2_AC-1, AUTH-2_AC-2, AUTH-2_AC-3, AUTH-2_AC-4
/// Axum middleware: checks for auth token in cookie first, then falls back
/// to `Authorization: Bearer <token>`. Verifies the JWT and injects
/// `AuthenticatedUser` into request extensions.
pub async fn require_auth(
    State(state): State<AppState>,
    jar: CookieJar,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // Try cookie first
    let token = jar
        .get(ACCESS_COOKIE)
        .map(|c| c.value().to_string())
        .or_else(|| {
            // Fall back to Authorization: Bearer header
            request
                .headers()
                .get(AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer ").map(|t| t.to_string()))
        })
        .ok_or_else(|| AppError::Unauthorized("Missing authentication".into()))?;

    // @zen-impl: AUTH-2_AC-4
    let claims = verify_access_token(&token, state.config.jwt_secret.as_bytes())
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired token".into()))?;

    // @zen-impl: AUTH-2_AC-2
    request.extensions_mut().insert(AuthenticatedUser(claims));

    Ok(next.run(request).await)
}

/// Axum middleware: requires the user to have an admin role.
pub async fn require_admin(
    State(state): State<AppState>,
    jar: CookieJar,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // Re-use auth extraction logic
    let token = jar
        .get(ACCESS_COOKIE)
        .map(|c| c.value().to_string())
        .or_else(|| {
            request
                .headers()
                .get(AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer ").map(|t| t.to_string()))
        })
        .ok_or_else(|| AppError::Unauthorized("Missing authentication".into()))?;

    let claims = verify_access_token(&token, state.config.jwt_secret.as_bytes())
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired token".into()))?;

    if !claims.roles.contains(&"admin".to_string()) {
        return Err(AppError::Forbidden("Admin access required".into()));
    }

    request.extensions_mut().insert(AuthenticatedUser(claims));

    Ok(next.run(request).await)
}
