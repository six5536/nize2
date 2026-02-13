// @zen-component: AUTH-TokenService
// @zen-component: AUTH-CredentialService
// @zen-component: AUTH-RefreshTokenStore
//
//! Authentication service — JWT token management, password hashing, login/register flows.

use std::path::PathBuf;

use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::distr::Alphanumeric;
use rand::{Rng, rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tracing::info;

use crate::error::{AppError, AppResult};
use crate::generated::models::{AuthStatusResponse, AuthUser, LogoutResponse, TokenResponse};

/// Access token lifetime: 15 minutes.
const ACCESS_TOKEN_EXPIRY_SECS: i64 = 15 * 60;

/// Refresh token lifetime: 30 days.
const REFRESH_TOKEN_EXPIRY_DAYS: i64 = 30;

/// bcrypt cost factor (matches ref code).
const BCRYPT_COST: u32 = 10;

/// JWT claims embedded in access tokens.
// @zen-impl: AUTH-1_AC-3
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    /// Subject — user ID (standard JWT `sub` claim).
    pub sub: String,
    /// User email.
    pub email: String,
    /// User roles (e.g. `["admin"]`).
    pub roles: Vec<String>,
    /// Expiry (unix timestamp).
    pub exp: i64,
    /// Issued at (unix timestamp).
    pub iat: i64,
}

// ---------------------------------------------------------------------------
// Password hashing
// ---------------------------------------------------------------------------

// @zen-impl: AUTH-1.1_AC-1
/// Hash a password with bcrypt (cost 10).
pub fn hash_password(password: &str) -> AppResult<String> {
    bcrypt::hash(password, BCRYPT_COST).map_err(|e| AppError::Internal(format!("bcrypt hash: {e}")))
}

// @zen-impl: AUTH-1.1_AC-1
/// Verify a password against a bcrypt hash.
pub fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
    bcrypt::verify(password, hash).map_err(|e| AppError::Internal(format!("bcrypt verify: {e}")))
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
// JWT generation & verification
// ---------------------------------------------------------------------------

// @zen-impl: AUTH-1_AC-1, AUTH-1_AC-3
/// Generate a signed JWT access token (HS256, 15 min expiry).
pub fn generate_access_token(
    user_id: &str,
    email: &str,
    roles: &[String],
    secret: &[u8],
) -> AppResult<String> {
    let now = Utc::now();
    let claims = TokenClaims {
        sub: user_id.to_string(),
        email: email.to_string(),
        roles: roles.to_vec(),
        exp: (now + Duration::seconds(ACCESS_TOKEN_EXPIRY_SECS)).timestamp(),
        iat: now.timestamp(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
    .map_err(|e| AppError::Internal(format!("jwt encode: {e}")))
}

// @zen-impl: AUTH-2_AC-4
/// Verify a JWT access token, returning the claims on success.
pub fn verify_access_token(token: &str, secret: &[u8]) -> Option<TokenClaims> {
    let key = DecodingKey::from_secret(secret);
    let mut validation = Validation::default();
    validation.validate_exp = true;
    decode::<TokenClaims>(token, &key, &validation)
        .ok()
        .map(|data| data.claims)
}

// ---------------------------------------------------------------------------
// JWT secret management
// ---------------------------------------------------------------------------

/// Resolve the JWT secret: env var `JWT_SECRET` → `AUTH_SECRET` → persisted file.
pub fn resolve_jwt_secret() -> String {
    if let Ok(secret) = std::env::var("JWT_SECRET") {
        if !secret.is_empty() {
            return secret;
        }
    }
    if let Ok(secret) = std::env::var("AUTH_SECRET") {
        if !secret.is_empty() {
            return secret;
        }
    }
    // Generate and persist
    let secret_path = jwt_secret_path();
    if let Ok(existing) = std::fs::read_to_string(&secret_path) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let secret: String = rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    if let Some(parent) = secret_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&secret_path, &secret);
    info!(path = %secret_path.display(), "generated new JWT secret");
    secret
}

/// Path to the persisted JWT secret file.
fn jwt_secret_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nize")
        .join("jwt-secret")
}

// ---------------------------------------------------------------------------
// DB operations
// ---------------------------------------------------------------------------

/// Fetch user roles from the `user_roles` table.
// @zen-impl: PRM-9_AC-1
async fn get_user_roles(pool: &PgPool, user_id: &str) -> AppResult<Vec<String>> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT role::text FROM user_roles WHERE user_id = $1::uuid",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
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
    let row = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT id::text, name, password_hash FROM users WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

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
    sqlx::query(
        "INSERT INTO refresh_tokens (token_hash, user_id, expires_at) VALUES ($1, $2::uuid, $3)",
    )
    .bind(&token_hash)
    .bind(&user_id)
    .bind(expires_at)
    .execute(pool)
    .await?;

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
    let exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
            .bind(email)
            .fetch_one(pool)
            .await?;
    if exists {
        return Err(AppError::Validation("Email already registered".into()));
    }

    // Check if this is the first user
    let user_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    let is_first_user = user_count == 0;

    // @zen-impl: AUTH-1.1_AC-1
    let pw_hash = hash_password(password)?;

    let user_id = sqlx::query_scalar::<_, String>(
        "INSERT INTO users (email, name, password_hash) VALUES ($1, $2, $3) RETURNING id::text",
    )
    .bind(email)
    .bind(name)
    .bind(&pw_hash)
    .fetch_one(pool)
    .await?;

    // @zen-impl: PRM-9_AC-1
    let mut roles = Vec::new();
    if is_first_user {
        sqlx::query("INSERT INTO user_roles (user_id, role) VALUES ($1::uuid, 'admin')")
            .bind(&user_id)
            .execute(pool)
            .await?;
        roles.push("admin".to_string());
        info!(email, "first user granted admin role");
    }

    let access_token = generate_access_token(&user_id, email, &roles, jwt_secret)?;
    let refresh_token = generate_refresh_token();
    let token_hash = hash_refresh_token(&refresh_token);

    let expires_at = Utc::now() + Duration::days(REFRESH_TOKEN_EXPIRY_DAYS);
    sqlx::query(
        "INSERT INTO refresh_tokens (token_hash, user_id, expires_at) VALUES ($1, $2::uuid, $3)",
    )
    .bind(&token_hash)
    .bind(&user_id)
    .bind(expires_at)
    .execute(pool)
    .await?;

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
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT rt.id::text, rt.user_id::text \
         FROM refresh_tokens rt \
         WHERE rt.token_hash = $1 \
           AND rt.revoked_at IS NULL \
           AND rt.expires_at > now()",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?;

    let (token_id, user_id) = match row {
        // @zen-impl: AUTH-3_AC-3
        None => return Err(AppError::Unauthorized("Invalid refresh token".into())),
        Some(r) => r,
    };

    // @zen-impl: AUTH-3_AC-4 — revoke old token
    sqlx::query("UPDATE refresh_tokens SET revoked_at = now() WHERE id = $1::uuid")
        .bind(&token_id)
        .execute(pool)
        .await?;

    // Fetch user
    let user = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT email, name FROM users WHERE id = $1::uuid",
    )
    .bind(&user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("User not found".into()))?;

    let (email, name) = user;
    let roles = get_user_roles(pool, &user_id).await?;

    // Issue new token pair
    let access_token = generate_access_token(&user_id, &email, &roles, jwt_secret)?;
    let new_refresh = generate_refresh_token();
    let new_hash = hash_refresh_token(&new_refresh);

    let expires_at = Utc::now() + Duration::days(REFRESH_TOKEN_EXPIRY_DAYS);
    sqlx::query(
        "INSERT INTO refresh_tokens (token_hash, user_id, expires_at) VALUES ($1, $2::uuid, $3)",
    )
    .bind(&new_hash)
    .bind(&user_id)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(build_token_response(
        &user_id,
        &email,
        name.as_deref(),
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
        sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = now() \
             WHERE token_hash = $1 AND revoked_at IS NULL",
        )
        .bind(&token_hash)
        .execute(pool)
        .await?;
    }
    Ok(LogoutResponse { success: true })
}

/// Logout all sessions — revoke all refresh tokens for a user.
pub async fn logout_all(pool: &PgPool, user_id: &str) -> AppResult<LogoutResponse> {
    sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = now() \
         WHERE user_id = $1::uuid AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(LogoutResponse { success: true })
}

/// Check whether an admin user exists (for first-run detection).
pub async fn admin_exists(pool: &PgPool) -> AppResult<AuthStatusResponse> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM user_roles WHERE role = 'admin')",
    )
    .fetch_one(pool)
    .await?;
    Ok(AuthStatusResponse {
        admin_exists: exists,
    })
}
