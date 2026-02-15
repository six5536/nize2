// @zen-component: AUTH-LoginEndpoint
// @zen-component: AUTH-RegistrationEndpoint
// @zen-component: AUTH-TokenRefreshEndpoint
//
//! Authentication request handlers.

use axum::Json;
use axum::extract::State;
use axum_extra::extract::CookieJar;

use crate::AppState;
use crate::error::AppResult;
use crate::generated::models::{
    AuthStatusResponse, LoginRequest, LogoutRequest, LogoutResponse, RefreshRequest,
    RegisterRequest, TokenResponse,
};
use crate::services::auth;
use crate::services::cookies;

// @zen-impl: AUTH-1_AC-1, AUTH-1_AC-2
/// `POST /auth/login` — authenticate with email + password.
/// Sets httpOnly auth cookies alongside the JSON response.
pub async fn login_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<LoginRequest>,
) -> AppResult<(CookieJar, Json<TokenResponse>)> {
    let resp = auth::login(
        &state.pool,
        &body.email,
        &body.password,
        state.config.jwt_secret.as_bytes(),
    )
    .await?;
    let jar = jar
        .add(cookies::access_cookie(&resp.access_token, resp.expires_in))
        .add(cookies::refresh_cookie(&resp.refresh_token));
    Ok((jar, Json(resp)))
}

// @zen-impl: AUTH-1.1_AC-2, AUTH-1.1_AC-4
/// `POST /auth/register` — create a new user account.
/// Sets httpOnly auth cookies alongside the JSON response.
pub async fn register_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<RegisterRequest>,
) -> AppResult<(CookieJar, Json<TokenResponse>)> {
    let resp = auth::register(
        &state.pool,
        &body.email,
        &body.password,
        body.name.as_deref(),
        state.config.jwt_secret.as_bytes(),
    )
    .await?;
    let jar = jar
        .add(cookies::access_cookie(&resp.access_token, resp.expires_in))
        .add(cookies::refresh_cookie(&resp.refresh_token));
    Ok((jar, Json(resp)))
}

// @zen-impl: AUTH-3_AC-1, AUTH-3_AC-2
/// `POST /auth/refresh` — exchange a refresh token for a new token pair.
/// Checks refresh token from cookie first, then from JSON body.
/// Sets new httpOnly auth cookies.
pub async fn refresh_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<RefreshRequest>,
) -> AppResult<(CookieJar, Json<TokenResponse>)> {
    // Prefer cookie refresh token, fall back to body
    let refresh_token = jar
        .get(cookies::REFRESH_COOKIE)
        .map(|c| c.value().to_string())
        .or(body.refresh_token)
        .ok_or_else(|| crate::error::AppError::Unauthorized("Missing refresh token".into()))?;

    let resp = auth::refresh(
        &state.pool,
        &refresh_token,
        state.config.jwt_secret.as_bytes(),
    )
    .await?;
    let jar = jar
        .add(cookies::access_cookie(&resp.access_token, resp.expires_in))
        .add(cookies::refresh_cookie(&resp.refresh_token));
    Ok((jar, Json(resp)))
}

// @zen-impl: AUTH-4_AC-1, AUTH-4_AC-2
/// `POST /auth/logout` — revoke a refresh token. Clears auth cookies.
pub async fn logout_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<LogoutRequest>,
) -> AppResult<(CookieJar, Json<LogoutResponse>)> {
    // Prefer cookie refresh token, fall back to body
    let refresh_token = jar
        .get(cookies::REFRESH_COOKIE)
        .map(|c| c.value().to_string())
        .or(body.refresh_token);

    let resp = auth::logout(&state.pool, refresh_token.as_deref()).await?;
    let jar = jar
        .add(cookies::clear_access_cookie())
        .add(cookies::clear_refresh_cookie());
    Ok((jar, Json(resp)))
}

/// `POST /auth/logout/all` — revoke all refresh tokens for the user (demo).
pub async fn logout_all_handler(
    jar: CookieJar,
) -> AppResult<(CookieJar, Json<serde_json::Value>)> {
    let jar = jar
        .add(cookies::clear_access_cookie())
        .add(cookies::clear_refresh_cookie());
    Ok((jar, Json(serde_json::json!({ "success": true }))))
}

/// `GET /auth/status` — check whether an admin user has been created.
pub async fn auth_status_handler(
    State(state): State<AppState>,
) -> AppResult<Json<AuthStatusResponse>> {
    let resp = auth::admin_exists(&state.pool).await?;
    Ok(Json(resp))
}
