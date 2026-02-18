//! Auth-related database queries.

use sqlx::PgPool;

use super::AuthError;
use crate::models::auth::User;
use crate::uuid::uuidv7;

/// Fetch a user by email, returning (id, name, password_hash).
pub async fn find_user_by_email(
    pool: &PgPool,
    email: &str,
) -> Result<Option<(String, Option<String>, Option<String>)>, AuthError> {
    let row = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT id::text, name, password_hash FROM users WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Create a new user, returning the user ID.
pub async fn create_user(
    pool: &PgPool,
    email: &str,
    name: Option<&str>,
    password_hash: &str,
) -> Result<String, AuthError> {
    let user_id = sqlx::query_scalar::<_, String>(
        "INSERT INTO users (email, name, password_hash) VALUES ($1, $2, $3) RETURNING id::text",
    )
    .bind(email)
    .bind(name)
    .bind(password_hash)
    .fetch_one(pool)
    .await?;
    Ok(user_id)
}

/// Fetch roles for a user.
pub async fn get_user_roles(pool: &PgPool, user_id: &str) -> Result<Vec<String>, AuthError> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT role::text FROM user_roles WHERE user_id = $1::uuid",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Grant a role to a user.
pub async fn grant_role(pool: &PgPool, user_id: &str, role: &str) -> Result<(), AuthError> {
    sqlx::query("INSERT INTO user_roles (user_id, role) VALUES ($1::uuid, $2::user_role)")
        .bind(user_id)
        .bind(role)
        .execute(pool)
        .await?;
    Ok(())
}

/// Check whether any admin user exists.
pub async fn admin_exists(pool: &PgPool) -> Result<bool, AuthError> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM user_roles WHERE role = 'admin')",
    )
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

/// Check whether an email is already registered.
pub async fn email_exists(pool: &PgPool, email: &str) -> Result<bool, AuthError> {
    let exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
            .bind(email)
            .fetch_one(pool)
            .await?;
    Ok(exists)
}

/// Count total users.
pub async fn user_count(pool: &PgPool) -> Result<i64, AuthError> {
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

/// Store a refresh token hash.
pub async fn store_refresh_token(
    pool: &PgPool,
    token_hash: &str,
    user_id: &str,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), AuthError> {
    sqlx::query(
        "INSERT INTO refresh_tokens (id, token_hash, user_id, expires_at) VALUES ($1, $2, $3::uuid, $4)",
    )
    .bind(uuidv7())
    .bind(token_hash)
    .bind(user_id)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Find a valid, non-revoked, non-expired refresh token. Returns (token_id, user_id).
pub async fn find_valid_refresh_token(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<(String, String)>, AuthError> {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT rt.id::text, rt.user_id::text \
         FROM refresh_tokens rt \
         WHERE rt.token_hash = $1 \
           AND rt.revoked_at IS NULL \
           AND rt.expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Revoke a refresh token by ID.
pub async fn revoke_refresh_token(pool: &PgPool, token_id: &str) -> Result<(), AuthError> {
    sqlx::query("UPDATE refresh_tokens SET revoked_at = now() WHERE id = $1::uuid")
        .bind(token_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Revoke a refresh token by hash.
pub async fn revoke_refresh_token_by_hash(
    pool: &PgPool,
    token_hash: &str,
) -> Result<(), AuthError> {
    sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = now() \
         WHERE token_hash = $1 AND revoked_at IS NULL",
    )
    .bind(token_hash)
    .execute(pool)
    .await?;
    Ok(())
}

/// Revoke all refresh tokens for a user.
pub async fn revoke_all_refresh_tokens(pool: &PgPool, user_id: &str) -> Result<(), AuthError> {
    sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = now() \
         WHERE user_id = $1::uuid AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetch user email and name by user ID.
pub async fn get_user_by_id(
    pool: &PgPool,
    user_id: &str,
) -> Result<Option<User>, AuthError> {
    let row = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT email, name FROM users WHERE id = $1::uuid",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(email, name)| User {
        id: user_id.to_string(),
        email,
        name,
    }))
}
