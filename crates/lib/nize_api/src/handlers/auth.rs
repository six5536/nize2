// @zen-component: AUTH-LoginEndpoint
// @zen-component: AUTH-RegistrationEndpoint
// @zen-component: AUTH-TokenRefreshEndpoint
//
//! Authentication request handlers.

use axum::Json;
use axum::extract::State;

use crate::AppState;
use crate::error::AppResult;
use crate::generated::models::{
    AuthStatusResponse, LoginRequest, LogoutRequest, LogoutResponse, RefreshRequest,
    RegisterRequest, TokenResponse,
};
use crate::services::auth;

// @zen-impl: AUTH-1_AC-1, AUTH-1_AC-2
/// `POST /auth/login` — authenticate with email + password.
pub async fn login_handler(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> AppResult<Json<TokenResponse>> {
    let resp = auth::login(
        &state.pool,
        &body.email,
        &body.password,
        state.config.jwt_secret.as_bytes(),
    )
    .await?;
    Ok(Json(resp))
}

// @zen-impl: AUTH-1.1_AC-2, AUTH-1.1_AC-4
/// `POST /auth/register` — create a new user account.
pub async fn register_handler(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> AppResult<Json<TokenResponse>> {
    let resp = auth::register(
        &state.pool,
        &body.email,
        &body.password,
        body.name.as_deref(),
        state.config.jwt_secret.as_bytes(),
    )
    .await?;
    Ok(Json(resp))
}

// @zen-impl: AUTH-3_AC-1, AUTH-3_AC-2
/// `POST /auth/refresh` — exchange a refresh token for a new token pair.
pub async fn refresh_handler(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> AppResult<Json<TokenResponse>> {
    let resp = auth::refresh(
        &state.pool,
        &body.refresh_token,
        state.config.jwt_secret.as_bytes(),
    )
    .await?;
    Ok(Json(resp))
}

// @zen-impl: AUTH-4_AC-1, AUTH-4_AC-2
/// `POST /auth/logout` — revoke a refresh token. Requires authentication.
pub async fn logout_handler(
    State(state): State<AppState>,
    Json(body): Json<LogoutRequest>,
) -> AppResult<Json<LogoutResponse>> {
    let resp = auth::logout(&state.pool, body.refresh_token.as_deref()).await?;
    Ok(Json(resp))
}

/// `GET /auth/status` — check whether an admin user has been created.
pub async fn auth_status_handler(
    State(state): State<AppState>,
) -> AppResult<Json<AuthStatusResponse>> {
    let resp = auth::admin_exists(&state.pool).await?;
    Ok(Json(resp))
}
