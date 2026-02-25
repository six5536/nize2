// @awa-component: CFG-ConfigService
//
//! Configuration service — orchestrates config operations for the API layer.

use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::RwLock;

use nize_core::config::ConfigError;
use nize_core::config::cache::ConfigCache;
use nize_core::config::queries;
use nize_core::config::resolver;
use nize_core::config::validation;
use nize_core::mcp::secrets;
use nize_core::models::config::{ConfigScope, ConfigValue, ResolvedConfigItem};

use crate::error::{AppError, AppResult};

// ---------------------------------------------------------------------------
// Error conversion
// ---------------------------------------------------------------------------

impl From<ConfigError> for AppError {
    fn from(e: ConfigError) -> Self {
        match e {
            ConfigError::NotFound(msg) => AppError::NotFound(msg),
            ConfigError::ValidationError(msg) => AppError::Validation(msg),
            ConfigError::DbError(e) => AppError::from(e),
        }
    }
}

// ---------------------------------------------------------------------------
// Secret helpers
// ---------------------------------------------------------------------------

/// Mask a secret value for API responses — show last 4 chars or empty.
fn mask_secret_value(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 4 {
        "••••".to_string()
    } else {
        let tail: String = chars[chars.len() - 4..].iter().collect();
        format!("••••••{tail}")
    }
}

/// Apply masking to resolved config items with `display_type = "secret"`.
fn mask_secret_items(items: Vec<ResolvedConfigItem>) -> Vec<ResolvedConfigItem> {
    items
        .into_iter()
        .map(|mut item| {
            if item.display_type == "secret" {
                item.value = mask_secret_value(&item.value);
            }
            item
        })
        .collect()
}

// ---------------------------------------------------------------------------
// User config operations
// ---------------------------------------------------------------------------

/// Get all config items resolved for a user.
// @awa-impl: PLAN-028-1.3
pub async fn get_user_config(
    pool: &PgPool,
    cache: &Arc<RwLock<ConfigCache>>,
    user_id: &str,
) -> AppResult<Vec<ResolvedConfigItem>> {
    let items = resolver::get_all_effective_values(pool, cache, Some(user_id)).await?;
    Ok(mask_secret_items(items))
}

/// Update a single user config override.
// @awa-impl: PLAN-028-1.2
pub async fn update_user_config(
    pool: &PgPool,
    cache: &Arc<RwLock<ConfigCache>>,
    user_id: &str,
    key: &str,
    value: &str,
    encryption_key: &str,
) -> AppResult<ResolvedConfigItem> {
    // Verify definition exists
    let def = queries::get_definition(pool, key)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Config key not found: {key}")))?;

    // Validate
    if let Some(ref validators) = def.validators {
        let errors = validation::validate_value(value, validators);
        if !errors.is_empty() {
            return Err(AppError::Validation(errors.join("; ")));
        }
    }

    // Encrypt secret values before storage
    let store_value = if def.display_type == "secret" && !value.is_empty() {
        secrets::encrypt(value, encryption_key)
            .map_err(|e| AppError::Internal(format!("Encryption failed: {e}")))?
    } else {
        value.to_string()
    };

    // Upsert
    let cv = queries::upsert_value(
        pool,
        key,
        &ConfigScope::UserOverride,
        Some(user_id),
        &store_value,
    )
    .await?;

    // Invalidate cache
    {
        let mut c = cache.write().await;
        c.invalidate(key, ConfigScope::UserOverride.as_str(), Some(user_id));
    }

    // Mask the response value for secret display types
    let mut result = ResolvedConfigItem::from_definition(&def, Some(&cv.value), true);
    if def.display_type == "secret" {
        result.value = mask_secret_value(value);
    }
    Ok(result)
}

/// Reset (delete) a user config override, reverting to system/default.
pub async fn reset_user_config(
    pool: &PgPool,
    cache: &Arc<RwLock<ConfigCache>>,
    user_id: &str,
    key: &str,
) -> AppResult<bool> {
    // Verify definition exists
    queries::get_definition(pool, key)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Config key not found: {key}")))?;

    let deleted =
        queries::delete_value(pool, key, &ConfigScope::UserOverride, Some(user_id)).await?;

    // Invalidate cache
    {
        let mut c = cache.write().await;
        c.invalidate(key, ConfigScope::UserOverride.as_str(), Some(user_id));
    }

    Ok(deleted)
}

// ---------------------------------------------------------------------------
// Admin config operations
// ---------------------------------------------------------------------------

/// Admin config item — includes scope metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdminConfigItem {
    pub key: String,
    pub category: String,
    #[serde(rename = "type")]
    pub value_type: String,
    #[serde(rename = "displayType")]
    pub display_type: String,
    #[serde(rename = "possibleValues")]
    pub possible_values: Option<Vec<String>>,
    #[serde(rename = "defaultValue")]
    pub default_value: String,
    pub label: Option<String>,
    pub description: Option<String>,
    /// All values across scopes.
    pub values: Vec<AdminConfigValue>,
}

/// A config value with scope info for admin views.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdminConfigValue {
    pub id: String,
    pub scope: String,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    pub value: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

/// Get admin config view — definitions with all values.
pub async fn get_admin_config(
    pool: &PgPool,
    scope: Option<&ConfigScope>,
    user_id: Option<&str>,
    category: Option<&str>,
    search: Option<&str>,
) -> AppResult<Vec<AdminConfigItem>> {
    let definitions = queries::get_all_definitions(pool).await?;
    let values = queries::get_all_values(pool, scope, user_id, category).await?;

    // Group values by key
    let mut values_by_key: std::collections::HashMap<String, Vec<&ConfigValue>> =
        std::collections::HashMap::new();
    for v in &values {
        values_by_key.entry(v.key.clone()).or_default().push(v);
    }

    let mut items: Vec<AdminConfigItem> = definitions
        .into_iter()
        .filter(|def| {
            if let Some(s) = search {
                let s = s.to_lowercase();
                def.key.to_lowercase().contains(&s)
                    || def.category.to_lowercase().contains(&s)
                    || def
                        .label
                        .as_ref()
                        .is_some_and(|l| l.to_lowercase().contains(&s))
                    || def
                        .description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&s))
            } else {
                true
            }
        })
        .filter(|def| {
            if let Some(cat) = category {
                def.category == cat
            } else {
                true
            }
        })
        .map(|def| {
            let is_secret = def.display_type == "secret";
            let vals = values_by_key
                .get(&def.key)
                .map(|vs| {
                    vs.iter()
                        .map(|v| AdminConfigValue {
                            id: v.id.clone(),
                            scope: v.scope.as_str().to_string(),
                            user_id: v.user_id.clone(),
                            value: if is_secret {
                                mask_secret_value(&v.value)
                            } else {
                                v.value.clone()
                            },
                            updated_at: v.updated_at.to_rfc3339(),
                        })
                        .collect()
                })
                .unwrap_or_default();

            AdminConfigItem {
                key: def.key,
                category: def.category,
                value_type: def.value_type,
                display_type: def.display_type,
                possible_values: def.possible_values,
                default_value: def.default_value,
                label: def.label,
                description: def.description,
                values: vals,
            }
        })
        .collect();

    items.sort_by(|a, b| a.key.cmp(&b.key));

    Ok(items)
}

/// Update an admin config value (system or user-override scope).
// @awa-impl: PLAN-028-1.2
pub async fn update_admin_config(
    pool: &PgPool,
    cache: &Arc<RwLock<ConfigCache>>,
    scope: &ConfigScope,
    key: &str,
    value: &str,
    user_id: Option<&str>,
    encryption_key: &str,
) -> AppResult<ConfigValue> {
    // Verify definition exists
    let def = queries::get_definition(pool, key)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Config key not found: {key}")))?;

    // Validate
    if let Some(ref validators) = def.validators {
        let errors = validation::validate_value(value, validators);
        if !errors.is_empty() {
            return Err(AppError::Validation(errors.join("; ")));
        }
    }

    // Encrypt secret values before storage
    let store_value = if def.display_type == "secret" && !value.is_empty() {
        secrets::encrypt(value, encryption_key)
            .map_err(|e| AppError::Internal(format!("Encryption failed: {e}")))?
    } else {
        value.to_string()
    };

    let cv = queries::upsert_value(pool, key, scope, user_id, &store_value).await?;

    // Invalidate cache
    {
        let mut c = cache.write().await;
        c.invalidate_all_for_key(key);
    }

    // Mask the stored value for secret display types
    let mut cv = cv;
    if def.display_type == "secret" {
        cv.value = mask_secret_value(value);
    }
    Ok(cv)
}

// ---------------------------------------------------------------------------
// Secret decryption (internal use only — AI proxy)
// ---------------------------------------------------------------------------

/// Resolve and decrypt a secret config value for a user.
///
/// Resolution order: user-override → system → env var → None.
/// Only works for `display_type = "secret"` definitions.
// @awa-impl: PLAN-028-1.4
pub async fn decrypt_secret_config_value(
    pool: &PgPool,
    _cache: &Arc<RwLock<ConfigCache>>,
    user_id: &str,
    key: &str,
    encryption_key: &str,
    env_fallback: Option<&str>,
) -> AppResult<Option<String>> {
    // Verify definition exists and is a secret
    let def = queries::get_definition(pool, key)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Config key not found: {key}")))?;
    if def.display_type != "secret" {
        return Err(AppError::Validation(format!(
            "Config key {key} is not a secret"
        )));
    }

    // Try user-override first
    if let Some(v) =
        queries::get_value(pool, key, &ConfigScope::UserOverride, Some(user_id)).await?
        && !v.value.is_empty()
    {
        let decrypted = secrets::decrypt(&v.value, encryption_key)
            .map_err(|e| AppError::Internal(format!("Decryption failed: {e}")))?;
        return Ok(Some(decrypted));
    }

    // Try system scope
    if let Some(v) = queries::get_value(pool, key, &ConfigScope::System, None).await?
        && !v.value.is_empty()
    {
        let decrypted = secrets::decrypt(&v.value, encryption_key)
            .map_err(|e| AppError::Internal(format!("Decryption failed: {e}")))?;
        return Ok(Some(decrypted));
    }

    // Env var fallback
    if let Some(env_var) = env_fallback
        && let Ok(val) = std::env::var(env_var)
        && !val.is_empty()
    {
        return Ok(Some(val));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    // @awa-test: PLAN-028-1.2 — mask hides all but last 4 chars
    #[test]
    fn mask_shows_last_four_chars() {
        assert_eq!(mask_secret_value("sk-abc123xyz"), "••••••3xyz");
    }

    // @awa-test: PLAN-028-1.2 — mask returns empty for empty input
    #[test]
    fn mask_empty_returns_empty() {
        assert_eq!(mask_secret_value(""), "");
    }

    // @awa-test: PLAN-028-1.2 — mask short values (<= 4 chars)
    #[test]
    fn mask_short_value_fully_masked() {
        assert_eq!(mask_secret_value("abcd"), "••••");
        assert_eq!(mask_secret_value("ab"), "••••");
        assert_eq!(mask_secret_value("a"), "••••");
    }

    // @awa-test: PLAN-028-1.2 — mask preserves exactly 4 trailing chars
    #[test]
    fn mask_preserves_trailing_four() {
        // "12345" has 5 chars, tail = chars[1..] = "2345"
        assert_eq!(mask_secret_value("12345"), "••••••2345");
    }

    // @awa-test: PLAN-028-1.2 — mask_secret_items applies to secret display type only
    #[test]
    fn mask_items_only_secrets() {
        let items = vec![
            ResolvedConfigItem {
                key: "agent.apiKey.anthropic".to_string(),
                value: "sk-ant-secret-key".to_string(),
                default_value: "".to_string(),
                display_type: "secret".to_string(),
                label: Some("Anthropic API Key".to_string()),
                description: None,
                category: "agent".to_string(),
                value_type: "string".to_string(),
                validators: None,
                possible_values: None,
                is_overridden: false,
            },
            ResolvedConfigItem {
                key: "agent.model".to_string(),
                value: "claude-sonnet-4-20250514".to_string(),
                default_value: "claude-sonnet-4-20250514".to_string(),
                display_type: "text".to_string(),
                label: Some("Model".to_string()),
                description: None,
                category: "agent".to_string(),
                value_type: "string".to_string(),
                validators: None,
                possible_values: None,
                is_overridden: false,
            },
        ];
        let masked = mask_secret_items(items);
        // Secret item should be masked
        assert_eq!(masked[0].value, "••••••-key");
        // Non-secret item should be unchanged
        assert_eq!(masked[1].value, "claude-sonnet-4-20250514");
    }

    // @awa-test: PLAN-028-1.1 — encrypt-on-write roundtrips correctly
    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = "test-encryption-key-for-roundtrip";
        let plaintext = "sk-ant-api03-secret";
        let encrypted = secrets::encrypt(plaintext, key).unwrap();
        assert_ne!(encrypted, plaintext);
        let decrypted = secrets::decrypt(&encrypted, key).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
