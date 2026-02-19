//! JWT token generation and verification.

use std::path::PathBuf;

use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::distr::Alphanumeric;
use rand::{Rng, rng};
use tracing::info;

use super::AuthError;
use crate::models::auth::TokenClaims;

/// Access token lifetime: 15 minutes.
const ACCESS_TOKEN_EXPIRY_SECS: i64 = 15 * 60;

// @zen-impl: AUTH-1_AC-1, AUTH-1_AC-3
/// Generate a signed JWT access token (HS256, 15 min expiry).
pub fn generate_access_token(
    user_id: &str,
    email: &str,
    roles: &[String],
    secret: &[u8],
) -> Result<String, AuthError> {
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
    .map_err(|e| AuthError::TokenError(format!("jwt encode: {e}")))
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

/// Resolve the JWT secret: env var `JWT_SECRET` → `AUTH_SECRET` → persisted file.
pub fn resolve_jwt_secret() -> String {
    if let Ok(secret) = std::env::var("JWT_SECRET")
        && !secret.is_empty()
    {
        return secret;
    }
    if let Ok(secret) = std::env::var("AUTH_SECRET")
        && !secret.is_empty()
    {
        return secret;
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
