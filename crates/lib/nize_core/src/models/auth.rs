//! Authentication domain models.
//!
//! These are internal domain models, distinct from API-specific generated models
//! (which have `#[serde(rename)]` for camelCase etc.).

use serde::{Deserialize, Serialize};

/// Domain user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
}

/// User with password hash (for internal auth flows).
#[derive(Debug, Clone)]
pub struct UserWithPassword {
    pub user: User,
    pub password_hash: Option<String>,
}

/// User role association.
#[derive(Debug, Clone)]
pub struct UserRole {
    pub user_id: String,
    pub role: String,
}

/// Refresh token record stored in the database.
#[derive(Debug, Clone)]
pub struct RefreshTokenRecord {
    pub id: String,
    pub user_id: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// JWT claims embedded in access tokens.
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

/// MCP API token record stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTokenRecord {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}
