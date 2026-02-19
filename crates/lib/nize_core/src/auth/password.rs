//! Password hashing via bcrypt.

use super::AuthError;

/// bcrypt cost factor.
const BCRYPT_COST: u32 = 10;

/// Hash a password with bcrypt (cost 10).
pub fn hash_password(password: &str) -> Result<String, AuthError> {
    bcrypt::hash(password, BCRYPT_COST)
        .map_err(|e| AuthError::Internal(format!("bcrypt hash: {e}")))
}

/// Verify a password against a bcrypt hash.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, AuthError> {
    bcrypt::verify(password, hash).map_err(|e| AuthError::Internal(format!("bcrypt verify: {e}")))
}
