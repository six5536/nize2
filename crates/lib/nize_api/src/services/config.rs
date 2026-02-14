// @zen-component: CFG-ConfigService
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
// User config operations
// ---------------------------------------------------------------------------

/// Get all config items resolved for a user.
pub async fn get_user_config(
    pool: &PgPool,
    cache: &Arc<RwLock<ConfigCache>>,
    user_id: &str,
) -> AppResult<Vec<ResolvedConfigItem>> {
    let items = resolver::get_all_effective_values(pool, cache, Some(user_id)).await?;
    Ok(items)
}

/// Update a single user config override.
pub async fn update_user_config(
    pool: &PgPool,
    cache: &Arc<RwLock<ConfigCache>>,
    user_id: &str,
    key: &str,
    value: &str,
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

    // Upsert
    let cv = queries::upsert_value(pool, key, &ConfigScope::UserOverride, Some(user_id), value).await?;

    // Invalidate cache
    {
        let mut c = cache.write().await;
        c.invalidate(key, ConfigScope::UserOverride.as_str(), Some(user_id));
    }

    Ok(ResolvedConfigItem::from_definition(&def, Some(&cv.value), true))
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

    let deleted = queries::delete_value(pool, key, &ConfigScope::UserOverride, Some(user_id)).await?;

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
                    || def.label.as_ref().is_some_and(|l| l.to_lowercase().contains(&s))
                    || def.description.as_ref().is_some_and(|d| d.to_lowercase().contains(&s))
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
            let vals = values_by_key
                .get(&def.key)
                .map(|vs| {
                    vs.iter()
                        .map(|v| AdminConfigValue {
                            id: v.id.clone(),
                            scope: v.scope.as_str().to_string(),
                            user_id: v.user_id.clone(),
                            value: v.value.clone(),
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
pub async fn update_admin_config(
    pool: &PgPool,
    cache: &Arc<RwLock<ConfigCache>>,
    scope: &ConfigScope,
    key: &str,
    value: &str,
    user_id: Option<&str>,
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

    let cv = queries::upsert_value(pool, key, scope, user_id, value).await?;

    // Invalidate cache
    {
        let mut c = cache.write().await;
        c.invalidate_all_for_key(key);
    }

    Ok(cv)
}
