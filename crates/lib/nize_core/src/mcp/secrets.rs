//! AES-256-GCM encryption for MCP server secrets.
//!
//! Provides encrypt/decrypt functions for API keys and OAuth client secrets.
//! Uses AES-256-GCM with random 12-byte nonces (prepended to ciphertext).
//! Output is base64-encoded for storage in TEXT columns.

use super::McpError;

use rand::RngCore;
use sha2::{Digest, Sha256};

/// Nonce size for AES-256-GCM (12 bytes).
const NONCE_SIZE: usize = 12;
/// AES-256 key size (32 bytes).
const KEY_SIZE: usize = 32;
/// GCM tag size (16 bytes).
const TAG_SIZE: usize = 16;

/// Derive a 32-byte key from a passphrase using SHA-256.
fn derive_key(passphrase: &str) -> [u8; KEY_SIZE] {
    let mut hasher = Sha256::new();
    hasher.update(passphrase.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; KEY_SIZE];
    key.copy_from_slice(&result);
    key
}

/// Encrypt plaintext with AES-256-GCM.
///
/// Returns base64-encoded `nonce || ciphertext || tag`.
pub fn encrypt(plaintext: &str, encryption_key: &str) -> Result<String, McpError> {
    use aes_gcm::aead::Aead;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};

    let key_bytes = derive_key(encryption_key);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| McpError::EncryptionError(format!("Key init failed: {e}")))?;

    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| McpError::EncryptionError(format!("Encryption failed: {e}")))?;

    // Prepend nonce to ciphertext
    let mut combined = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(&combined))
}

/// Decrypt base64-encoded `nonce || ciphertext || tag`.
pub fn decrypt(encrypted_b64: &str, encryption_key: &str) -> Result<String, McpError> {
    use aes_gcm::aead::Aead;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
    use base64::Engine;

    let combined = base64::engine::general_purpose::STANDARD
        .decode(encrypted_b64)
        .map_err(|e| McpError::EncryptionError(format!("Base64 decode failed: {e}")))?;

    if combined.len() < NONCE_SIZE + TAG_SIZE {
        return Err(McpError::EncryptionError("Ciphertext too short".into()));
    }

    let key_bytes = derive_key(encryption_key);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| McpError::EncryptionError(format!("Key init failed: {e}")))?;

    let nonce = Nonce::from_slice(&combined[..NONCE_SIZE]);
    let ciphertext = &combined[NONCE_SIZE..];

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| McpError::EncryptionError(format!("Decryption failed: {e}")))?;

    String::from_utf8(plaintext)
        .map_err(|e| McpError::EncryptionError(format!("UTF-8 decode failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_round_trip() {
        let key = "test-encryption-key-for-nize";
        let plaintext = "sk-super-secret-api-key-12345";
        let encrypted = encrypt(plaintext, key).unwrap();
        let decrypted = decrypt(&encrypted, key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn wrong_key_fails() {
        let key = "correct-key";
        let wrong_key = "wrong-key";
        let plaintext = "secret";
        let encrypted = encrypt(plaintext, key).unwrap();
        assert!(decrypt(&encrypted, wrong_key).is_err());
    }

    #[test]
    fn empty_plaintext() {
        let key = "test-key";
        let encrypted = encrypt("", key).unwrap();
        let decrypted = decrypt(&encrypted, key).unwrap();
        assert_eq!(decrypted, "");
    }
}
