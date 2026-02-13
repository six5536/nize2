// @zen-component: AUTH-AccessControl
//
//! Authentication middleware — Bearer token extraction and JWT verification.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use axum::http::header::AUTHORIZATION;

use crate::AppState;
use crate::error::AppError;
use crate::services::auth::{TokenClaims, verify_access_token};

/// Key used to store `TokenClaims` in request extensions.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser(pub TokenClaims);

// @zen-impl: AUTH-2_AC-1, AUTH-2_AC-2, AUTH-2_AC-3, AUTH-2_AC-4
/// Axum middleware: extracts `Authorization: Bearer <token>`, verifies the JWT,
/// and injects `AuthenticatedUser` into request extensions.
pub async fn require_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing authorization header".into()))?;

    // @zen-impl: AUTH-2_AC-1
    let token = header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::Unauthorized("Invalid authorization scheme".into()))?;

    // @zen-impl: AUTH-2_AC-4
    let claims = verify_access_token(token, state.config.jwt_secret.as_bytes())
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired token".into()))?;

    // @zen-impl: AUTH-2_AC-2
    request.extensions_mut().insert(AuthenticatedUser(claims));

    Ok(next.run(request).await)
}
