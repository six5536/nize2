// @zen-component: AUTH-TokenService
// @zen-component: AUTH-CredentialService
// @zen-component: AUTH-RefreshTokenStore
//
//! Authentication service — login/register flows delegating to `nize_core::auth`.

use chrono::{Duration, Utc};
use rand::distr::Alphanumeric;
use rand::{Rng, rng};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tracing::info;

use crate::error::{AppError, AppResult};
use crate::generated::models::{AuthStatusResponse, AuthUser, LogoutResponse, TokenResponse};

// Re-export from nize_core for backward compatibility.
pub use nize_core::auth::jwt::{resolve_jwt_secret, verify_access_token};
pub use nize_core::models::auth::TokenClaims;

/// Access token lifetime: 15 minutes.
const ACCESS_TOKEN_EXPIRY_SECS: i64 = 15 * 60;

/// Refresh token lifetime: 30 days.
const REFRESH_TOKEN_EXPIRY_DAYS: i64 = 30;

// ---------------------------------------------------------------------------
// Password hashing (delegate to nize_core)
// ---------------------------------------------------------------------------

// @zen-impl: AUTH-1.1_AC-1
/// Hash a password with bcrypt (cost 10).
pub fn hash_password(password: &str) -> AppResult<String> {
    nize_core::auth::password::hash_password(password).map_err(AppError::from)
}

// @zen-impl: AUTH-1.1_AC-1
/// Verify a password against a bcrypt hash.
pub fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
    nize_core::auth::password::verify_password(password, hash).map_err(AppError::from)
}

// ---------------------------------------------------------------------------
// Refresh token generation & hashing
// ---------------------------------------------------------------------------

/// Generate a cryptographically random refresh token (64 alphanumeric chars).
fn generate_refresh_token() -> String {
    rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect()
}

/// SHA-256 hash a refresh token for storage.
fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ---------------------------------------------------------------------------
// JWT generation & verification (delegate to nize_core)
// ---------------------------------------------------------------------------

// @zen-impl: AUTH-1_AC-1, AUTH-1_AC-3
/// Generate a signed JWT access token (HS256, 15 min expiry).
pub fn generate_access_token(
    user_id: &str,
    email: &str,
    roles: &[String],
    secret: &[u8],
) -> AppResult<String> {
    nize_core::auth::jwt::generate_access_token(user_id, email, roles, secret)
        .map_err(AppError::from)
}

// ---------------------------------------------------------------------------
// DB operations
// ---------------------------------------------------------------------------

/// Fetch user roles from the `user_roles` table.
// @zen-impl: PRM-9_AC-1
async fn get_user_roles(pool: &PgPool, user_id: &str) -> AppResult<Vec<String>> {
    nize_core::auth::queries::get_user_roles(pool, user_id)
        .await
        .map_err(AppError::from)
}

/// Build a `TokenResponse` from user data plus a fresh token pair.
fn build_token_response(
    user_id: &str,
    email: &str,
    name: Option<&str>,
    roles: &[String],
    access_token: String,
    refresh_token: String,
) -> TokenResponse {
    TokenResponse {
        access_token,
        refresh_token,
        expires_in: ACCESS_TOKEN_EXPIRY_SECS,
        token_type: "Bearer".to_string(),
        user: AuthUser {
            id: user_id.to_string(),
            email: email.to_string(),
            name: name.map(|n| n.to_string()),
            roles: roles.to_vec(),
        },
    }
}

// ---------------------------------------------------------------------------
// Public auth operations
// ---------------------------------------------------------------------------

// @zen-impl: AUTH-1_AC-1, AUTH-1_AC-2
/// Authenticate with email + password.
pub async fn login(
    pool: &PgPool,
    email: &str,
    password: &str,
    jwt_secret: &[u8],
) -> AppResult<TokenResponse> {
    let row = nize_core::auth::queries::find_user_by_email(pool, email).await?;

    let (user_id, name, pw_hash) = match row {
        // @zen-impl: AUTH-1_AC-2 — generic error for wrong email
        None => return Err(AppError::Unauthorized("Invalid credentials".into())),
        Some(r) => r,
    };

    let pw_hash = match pw_hash {
        None => return Err(AppError::Unauthorized("Invalid credentials".into())),
        Some(h) => h,
    };

    // @zen-impl: AUTH-1_AC-2 — generic error for wrong password
    if !verify_password(password, &pw_hash)? {
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    }

    let roles = get_user_roles(pool, &user_id).await?;
    let access_token = generate_access_token(&user_id, email, &roles, jwt_secret)?;
    let refresh_token = generate_refresh_token();
    let token_hash = hash_refresh_token(&refresh_token);

    // @zen-impl: AUTH-1_AC-4
    let expires_at = Utc::now() + Duration::days(REFRESH_TOKEN_EXPIRY_DAYS);
    nize_core::auth::queries::store_refresh_token(pool, &token_hash, &user_id, expires_at).await?;

    Ok(build_token_response(
        &user_id,
        email,
        name.as_deref(),
        &roles,
        access_token,
        refresh_token,
    ))
}

// @zen-impl: AUTH-1.1_AC-2, AUTH-1.1_AC-4
// @zen-impl: PRM-9_AC-1 — first user is admin
/// Register a new user account. First user gets admin role.
pub async fn register(
    pool: &PgPool,
    email: &str,
    password: &str,
    name: Option<&str>,
    jwt_secret: &[u8],
) -> AppResult<TokenResponse> {
    // @zen-impl: AUTH-1.1_AC-2
    if password.len() < 8 {
        return Err(AppError::Validation(
            "Password must be at least 8 characters".into(),
        ));
    }

    // Check duplicate email
    if nize_core::auth::queries::email_exists(pool, email).await? {
        return Err(AppError::Validation("Email already registered".into()));
    }

    // Check if this is the first user
    let is_first_user = nize_core::auth::queries::user_count(pool).await? == 0;

    // @zen-impl: AUTH-1.1_AC-1
    let pw_hash = hash_password(password)?;

    let user_id = nize_core::auth::queries::create_user(pool, email, name, &pw_hash).await?;

    // @zen-impl: PRM-9_AC-1
    let mut roles = Vec::new();
    if is_first_user {
        nize_core::auth::queries::grant_role(pool, &user_id, "admin").await?;
        roles.push("admin".to_string());
        info!(email, "first user granted admin role");
    }

    let access_token = generate_access_token(&user_id, email, &roles, jwt_secret)?;
    let refresh_token = generate_refresh_token();
    let token_hash = hash_refresh_token(&refresh_token);

    let expires_at = Utc::now() + Duration::days(REFRESH_TOKEN_EXPIRY_DAYS);
    nize_core::auth::queries::store_refresh_token(pool, &token_hash, &user_id, expires_at).await?;

    Ok(build_token_response(
        &user_id,
        email,
        name,
        &roles,
        access_token,
        refresh_token,
    ))
}

// @zen-impl: AUTH-3_AC-1, AUTH-3_AC-2, AUTH-3_AC-4
/// Refresh an access token using a refresh token (single-use rotation).
pub async fn refresh(
    pool: &PgPool,
    refresh_token: &str,
    jwt_secret: &[u8],
) -> AppResult<TokenResponse> {
    let token_hash = hash_refresh_token(refresh_token);

    // Find valid, non-revoked, non-expired token
    let row = nize_core::auth::queries::find_valid_refresh_token(pool, &token_hash).await?;

    let (token_id, user_id) = match row {
        // @zen-impl: AUTH-3_AC-3
        None => return Err(AppError::Unauthorized("Invalid refresh token".into())),
        Some(r) => r,
    };

    // @zen-impl: AUTH-3_AC-4 — revoke old token
    nize_core::auth::queries::revoke_refresh_token(pool, &token_id).await?;

    // Fetch user
    let user = nize_core::auth::queries::get_user_by_id(pool, &user_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("User not found".into()))?;

    let roles = get_user_roles(pool, &user_id).await?;

    // Issue new token pair
    let access_token = generate_access_token(&user_id, &user.email, &roles, jwt_secret)?;
    let new_refresh = generate_refresh_token();
    let new_hash = hash_refresh_token(&new_refresh);

    let expires_at = Utc::now() + Duration::days(REFRESH_TOKEN_EXPIRY_DAYS);
    nize_core::auth::queries::store_refresh_token(pool, &new_hash, &user_id, expires_at).await?;

    Ok(build_token_response(
        &user_id,
        &user.email,
        user.name.as_deref(),
        &roles,
        access_token,
        new_refresh,
    ))
}

// @zen-impl: AUTH-4_AC-1, AUTH-4_AC-2
/// Logout — revoke a specific refresh token.
pub async fn logout(pool: &PgPool, refresh_token: Option<&str>) -> AppResult<LogoutResponse> {
    if let Some(token) = refresh_token {
        let token_hash = hash_refresh_token(token);
        nize_core::auth::queries::revoke_refresh_token_by_hash(pool, &token_hash).await?;
    }
    Ok(LogoutResponse { success: true })
}

/// Logout all sessions — revoke all refresh tokens for a user.
pub async fn logout_all(pool: &PgPool, user_id: &str) -> AppResult<LogoutResponse> {
    nize_core::auth::queries::revoke_all_refresh_tokens(pool, user_id).await?;
    Ok(LogoutResponse { success: true })
}

/// Check whether an admin user exists (for first-run detection).
pub async fn admin_exists(pool: &PgPool) -> AppResult<AuthStatusResponse> {
    let exists = nize_core::auth::queries::admin_exists(pool).await?;
    Ok(AuthStatusResponse {
        admin_exists: exists,
    })
}
