//! MCP API token management.
//!
//! Long-lived bearer tokens for MCP client authentication.

use rand::distr::Alphanumeric;
use rand::{Rng, rng};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use super::AuthError;
use crate::models::auth::{McpTokenRecord, User};
use crate::uuid::uuidv7;

/// Generate a random token (64 alphanumeric chars).
fn generate_token() -> String {
    rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect()
}

/// SHA-256 hash a token for storage.
fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Create a new MCP token for a user. Returns (plaintext_token, record).
///
/// When `overwrite` is true, any existing active (non-revoked) token with the
/// same name for this user is revoked before creating the new one.
/// When `overwrite` is false, returns an error if an active token with the same
/// name already exists.
pub async fn create_mcp_token(
    pool: &PgPool,
    user_id: &str,
    name: &str,
    overwrite: bool,
) -> Result<(String, McpTokenRecord), AuthError> {
    if overwrite {
        // Revoke any existing active token with the same name for this user
        sqlx::query(
            "UPDATE mcp_tokens SET revoked_at = now() \
             WHERE user_id = $1::uuid AND name = $2 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .bind(name)
        .execute(pool)
        .await?;
    } else {
        // Check for existing active token with same name
        let existing = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM mcp_tokens \
             WHERE user_id = $1::uuid AND name = $2 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .bind(name)
        .fetch_one(pool)
        .await?;

        if existing.0 > 0 {
            return Err(AuthError::ValidationError(format!(
                "An active token with name '{name}' already exists. Use overwrite=true to replace it."
            )));
        }
    }

    let plaintext = generate_token();
    let token_hash = hash_token(&plaintext);

    let row = sqlx::query_as::<_, (String, chrono::DateTime<chrono::Utc>)>(
        "INSERT INTO mcp_tokens (id, user_id, token_hash, name) \
         VALUES ($1, $2::uuid, $3, $4) \
         RETURNING id::text, created_at",
    )
    .bind(uuidv7())
    .bind(user_id)
    .bind(&token_hash)
    .bind(name)
    .fetch_one(pool)
    .await?;

    let record = McpTokenRecord {
        id: row.0,
        user_id: user_id.to_string(),
        name: name.to_string(),
        created_at: row.1,
        expires_at: None,
        revoked_at: None,
    };

    Ok((plaintext, record))
}

/// Validate an MCP bearer token. Returns the associated user if valid.
pub async fn validate_mcp_token(pool: &PgPool, token: &str) -> Result<Option<User>, AuthError> {
    let token_hash = hash_token(token);

    let row = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT u.id::text, u.email, u.name \
         FROM mcp_tokens mt \
         JOIN users u ON u.id = mt.user_id \
         WHERE mt.token_hash = $1 \
           AND mt.revoked_at IS NULL \
           AND (mt.expires_at IS NULL OR mt.expires_at > now())",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(id, email, name)| User { id, email, name }))
}

/// Revoke an MCP token by ID.
pub async fn revoke_mcp_token(pool: &PgPool, token_id: &str) -> Result<(), AuthError> {
    sqlx::query("UPDATE mcp_tokens SET revoked_at = now() WHERE id = $1::uuid")
        .bind(token_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// List MCP tokens for a user (without exposing hashes).
pub async fn list_mcp_tokens(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<McpTokenRecord>, AuthError> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            chrono::DateTime<chrono::Utc>,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<chrono::DateTime<chrono::Utc>>,
        ),
    >(
        "SELECT id::text, user_id::text, name, created_at, expires_at, revoked_at \
         FROM mcp_tokens \
         WHERE user_id = $1::uuid \
         ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, user_id, name, created_at, expires_at, revoked_at)| McpTokenRecord {
                id,
                user_id,
                name,
                created_at,
                expires_at,
                revoked_at,
            },
        )
        .collect())
}
