// @zen-component: PLAN-031-OAuthCore
//
//! Google OAuth support for MCP servers.
//!
//! Provides PKCE state management, token exchange, and token refresh for
//! Google OAuth flows used with gogmcp servers.

use std::time::{Duration, Instant};

use dashmap::DashMap;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tracing::debug;

use super::McpError;

/// TTL for PKCE state entries (10 minutes).
const STATE_TTL: Duration = Duration::from_secs(600);

/// Preemptive refresh threshold — refresh when 80% of token lifetime has passed.
const REFRESH_THRESHOLD_PERCENT: f64 = 0.80;

// =============================================================================
// PKCE helpers
// =============================================================================

/// Generate a cryptographic PKCE code verifier (43–128 chars, URL-safe).
pub fn generate_code_verifier() -> String {
    use base64::Engine;
    use rand::RngCore;

    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Compute S256 code challenge from a code verifier.
pub fn compute_code_challenge(verifier: &str) -> String {
    use base64::Engine;

    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Generate a cryptographic state parameter (CSRF token).
pub fn generate_state() -> String {
    use base64::Engine;
    use rand::RngCore;

    let mut bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

// =============================================================================
// PKCE state store
// =============================================================================

/// Pending OAuth state stored between initiate and callback.
pub struct OAuthPendingState {
    pub server_id: String,
    pub user_id: String,
    pub pkce_verifier: String,
    pub oauth_config_json: serde_json::Value,
    pub client_secret: String,
    pub redirect_uri: String,
    pub created_at: Instant,
}

/// In-memory store for OAuth PKCE state (keyed by state parameter).
pub struct OAuthStateStore {
    states: DashMap<String, OAuthPendingState>,
}

impl OAuthStateStore {
    pub fn new() -> Self {
        Self {
            states: DashMap::new(),
        }
    }

    /// Insert a pending state entry.
    pub fn insert(&self, state_key: String, pending: OAuthPendingState) {
        self.states.insert(state_key, pending);
    }

    /// Take (remove and return) a pending state entry.
    /// Returns `None` if not found or expired.
    pub fn take(&self, state_key: &str) -> Option<OAuthPendingState> {
        let (_, pending) = self.states.remove(state_key)?;
        if pending.created_at.elapsed() > STATE_TTL {
            return None;
        }
        Some(pending)
    }

    /// Evict expired entries.
    pub fn cleanup(&self) {
        self.states
            .retain(|_, v| v.created_at.elapsed() <= STATE_TTL);
    }

    /// Spawn a periodic cleanup task.
    pub fn spawn_cleanup_task(self: &std::sync::Arc<Self>) -> tokio::task::JoinHandle<()> {
        let store = std::sync::Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                store.cleanup();
            }
        })
    }
}

impl Default for OAuthStateStore {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Google token exchange
// =============================================================================

/// Response from Google's token endpoint.
#[derive(Debug, Deserialize)]
pub struct GoogleTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub scope: Option<String>,
}

/// Exchange an authorization code for Google tokens.
// @zen-impl: PLAN-031 Phase 5.2 — token exchange
pub async fn exchange_authorization_code(
    token_url: &str,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<GoogleTokenResponse, McpError> {
    let client = reqwest::Client::new();

    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("redirect_uri", redirect_uri),
        ("code_verifier", code_verifier),
    ];

    let resp = client
        .post(token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| McpError::ConnectionFailed(format!("Token exchange failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(McpError::ConnectionFailed(format!(
            "Token exchange HTTP {status}: {body}"
        )));
    }

    resp.json::<GoogleTokenResponse>()
        .await
        .map_err(|e| McpError::ConnectionFailed(format!("Token response parse error: {e}")))
}

// =============================================================================
// Token refresh
// =============================================================================

/// Refresh Google tokens using a refresh_token.
// @zen-impl: PLAN-031 Phase 6.1 — token refresh
pub async fn refresh_google_tokens(
    token_url: &str,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<GoogleTokenResponse, McpError> {
    let client = reqwest::Client::new();

    let params = [
        ("grant_type", "refresh_token"),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("refresh_token", refresh_token),
    ];

    let resp = client
        .post(token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| McpError::ConnectionFailed(format!("Token refresh failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(McpError::ConnectionFailed(format!(
            "Token refresh HTTP {status}: {body}"
        )));
    }

    resp.json::<GoogleTokenResponse>()
        .await
        .map_err(|e| McpError::ConnectionFailed(format!("Token refresh parse error: {e}")))
}

/// Check whether tokens should be refreshed (>80% of lifetime elapsed).
pub fn should_refresh(expires_at: &chrono::DateTime<chrono::Utc>) -> bool {
    let now = chrono::Utc::now();
    if *expires_at <= now {
        return true; // Already expired
    }
    // Estimate original token lifetime as 1 hour (Google default)
    let total_lifetime = Duration::from_secs(3600);
    let remaining = (*expires_at - now)
        .to_std()
        .unwrap_or(Duration::from_secs(0));
    let elapsed_fraction = 1.0 - (remaining.as_secs_f64() / total_lifetime.as_secs_f64());
    debug!(elapsed_fraction = elapsed_fraction, "Token refresh check");
    elapsed_fraction >= REFRESH_THRESHOLD_PERCENT
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // @zen-test: PLAN-031 Phase 9.1 — PKCE code verifier generation
    #[test]
    fn code_verifier_is_url_safe_and_sufficient_length() {
        let verifier = generate_code_verifier();
        assert!(
            verifier.len() >= 43,
            "verifier too short: {}",
            verifier.len()
        );
        assert!(
            verifier
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "verifier contains non-URL-safe chars: {verifier}"
        );
    }

    // @zen-test: PLAN-031 Phase 9.1 — PKCE S256 code challenge
    #[test]
    fn code_challenge_is_s256_of_verifier() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = compute_code_challenge(verifier);
        // RFC 7636 test vector
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    // @zen-test: PLAN-031 Phase 9.1 — state generation uniqueness
    #[test]
    fn generate_state_produces_unique_values() {
        let s1 = generate_state();
        let s2 = generate_state();
        assert_ne!(s1, s2);
        assert!(s1.len() >= 20);
    }

    // @zen-test: PLAN-031 Phase 9.1 — state store insert and take
    #[test]
    fn state_store_insert_and_take() {
        let store = OAuthStateStore::new();
        store.insert(
            "test-key".into(),
            OAuthPendingState {
                server_id: "srv1".into(),
                user_id: "usr1".into(),
                pkce_verifier: "verifier".into(),
                oauth_config_json: serde_json::json!({}),
                client_secret: "secret".into(),
                redirect_uri: "http://localhost/callback".into(),
                created_at: Instant::now(),
            },
        );

        let taken = store.take("test-key");
        assert!(taken.is_some());
        assert_eq!(taken.unwrap().server_id, "srv1");

        // Second take returns None (consumed)
        assert!(store.take("test-key").is_none());
    }

    // @zen-test: PLAN-031 Phase 9.1 — state store expired entry
    #[test]
    fn state_store_expired_entry_returns_none() {
        let store = OAuthStateStore::new();
        store.insert(
            "old-key".into(),
            OAuthPendingState {
                server_id: "srv1".into(),
                user_id: "usr1".into(),
                pkce_verifier: "verifier".into(),
                oauth_config_json: serde_json::json!({}),
                client_secret: "secret".into(),
                redirect_uri: "http://localhost/callback".into(),
                created_at: Instant::now() - Duration::from_secs(700), // past TTL
            },
        );

        assert!(store.take("old-key").is_none());
    }

    // @zen-test: PLAN-031 Phase 9.1 — state store cleanup
    #[test]
    fn state_store_cleanup_removes_expired() {
        let store = OAuthStateStore::new();
        store.insert(
            "fresh".into(),
            OAuthPendingState {
                server_id: "srv1".into(),
                user_id: "usr1".into(),
                pkce_verifier: "v".into(),
                oauth_config_json: serde_json::json!({}),
                client_secret: "s".into(),
                redirect_uri: "http://localhost/callback".into(),
                created_at: Instant::now(),
            },
        );
        store.insert(
            "stale".into(),
            OAuthPendingState {
                server_id: "srv2".into(),
                user_id: "usr1".into(),
                pkce_verifier: "v".into(),
                oauth_config_json: serde_json::json!({}),
                client_secret: "s".into(),
                redirect_uri: "http://localhost/callback".into(),
                created_at: Instant::now() - Duration::from_secs(700),
            },
        );

        store.cleanup();
        assert!(store.take("fresh").is_some());
        // "stale" was already cleaned up
        assert!(store.take("stale").is_none());
    }

    // @zen-test: PLAN-031 Phase 9.1 — spawn_cleanup_task compiles
    #[tokio::test]
    async fn spawn_cleanup_task_runs() {
        let store = Arc::new(OAuthStateStore::new());
        let handle = store.spawn_cleanup_task();
        // Let it tick once
        tokio::time::sleep(Duration::from_millis(50)).await;
        handle.abort();
    }

    // @zen-test: PLAN-031 Phase 6.1 — should_refresh logic
    #[test]
    fn should_refresh_returns_true_when_expired() {
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        assert!(should_refresh(&past));
    }

    // @zen-test: PLAN-031 Phase 6.1 — should_refresh logic
    #[test]
    fn should_refresh_returns_false_when_fresh() {
        let future = chrono::Utc::now() + chrono::Duration::hours(1);
        assert!(!should_refresh(&future));
    }

    // @zen-test: PLAN-031 Phase 6.1 — should_refresh near expiry
    #[test]
    fn should_refresh_returns_true_near_expiry() {
        // 5 minutes remaining out of 1 hour = ~92% elapsed
        let near = chrono::Utc::now() + chrono::Duration::minutes(5);
        assert!(should_refresh(&near));
    }
}
