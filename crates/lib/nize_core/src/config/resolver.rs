// @zen-component: CFG-ConfigResolver
//
//! Config value resolution — user-override → system → defaultValue.

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::RwLock;

use super::ConfigError;
use super::cache::ConfigCache;
use super::queries;
use crate::models::config::{ConfigScope, ResolvedConfigItem};

/// Resolve the effective value for a single config key.
///
/// Resolution order: user-override → system → defaultValue.
pub async fn get_effective_value(
    pool: &PgPool,
    cache: &Arc<RwLock<ConfigCache>>,
    key: &str,
    user_id: Option<&str>,
) -> Result<ResolvedConfigItem, ConfigError> {
    let def = queries::get_definition(pool, key)
        .await?
        .ok_or_else(|| ConfigError::NotFound(key.to_string()))?;

    let (value, is_overridden) = resolve_value(pool, cache, key, user_id).await?;

    Ok(ResolvedConfigItem::from_definition(
        &def,
        Some(&value),
        is_overridden,
    ))
}

/// Resolve all config definitions with effective values for a given user.
pub async fn get_all_effective_values(
    pool: &PgPool,
    cache: &Arc<RwLock<ConfigCache>>,
    user_id: Option<&str>,
) -> Result<Vec<ResolvedConfigItem>, ConfigError> {
    let definitions = queries::get_all_definitions(pool).await?;

    // Bulk-fetch system and user values to avoid N+1
    let system_values = queries::get_system_values(pool).await?;
    let user_values = match user_id {
        Some(uid) => queries::get_user_values(pool, uid).await?,
        None => Vec::new(),
    };

    // Index by key for fast lookup
    let system_map: HashMap<&str, &str> = system_values
        .iter()
        .map(|v| (v.key.as_str(), v.value.as_str()))
        .collect();
    let user_map: HashMap<&str, &str> = user_values
        .iter()
        .map(|v| (v.key.as_str(), v.value.as_str()))
        .collect();

    // Update cache with fetched values
    {
        let mut c = cache.write().await;
        for v in &system_values {
            c.set(&v.key, ConfigScope::System.as_str(), None, v.value.clone());
        }
        for v in &user_values {
            c.set(
                &v.key,
                ConfigScope::UserOverride.as_str(),
                v.user_id.as_deref(),
                v.value.clone(),
            );
        }
    }

    let items = definitions
        .iter()
        .map(|def| {
            // user-override → system → defaultValue
            let (value, is_overridden) = if let Some(uv) = user_map.get(def.key.as_str()) {
                (uv.to_string(), true)
            } else if let Some(sv) = system_map.get(def.key.as_str()) {
                (sv.to_string(), false)
            } else {
                (def.default_value.clone(), false)
            };
            ResolvedConfigItem::from_definition(def, Some(&value), is_overridden)
        })
        .collect();

    Ok(items)
}

/// Resolve a single system-scope value (no user context).
pub async fn get_system_value(
    pool: &PgPool,
    cache: &Arc<RwLock<ConfigCache>>,
    key: &str,
) -> Result<String, ConfigError> {
    // Check cache first
    {
        let c = cache.read().await;
        if let Some(v) = c.get(key, ConfigScope::System.as_str(), None) {
            return Ok(v);
        }
    }

    // Fetch from DB
    let val = queries::get_value(pool, key, &ConfigScope::System, None).await?;

    if let Some(v) = val {
        let mut c = cache.write().await;
        c.set(key, ConfigScope::System.as_str(), None, v.value.clone());
        return Ok(v.value);
    }

    // Fall back to definition default
    let def = queries::get_definition(pool, key)
        .await?
        .ok_or_else(|| ConfigError::NotFound(key.to_string()))?;
    Ok(def.default_value)
}

// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

/// Resolve a value through the hierarchy, returning (value, is_overridden).
async fn resolve_value(
    pool: &PgPool,
    cache: &Arc<RwLock<ConfigCache>>,
    key: &str,
    user_id: Option<&str>,
) -> Result<(String, bool), ConfigError> {
    // 1. Try user-override (if user_id given)
    if let Some(uid) = user_id {
        // Check cache
        {
            let c = cache.read().await;
            if let Some(v) = c.get(key, ConfigScope::UserOverride.as_str(), Some(uid)) {
                return Ok((v, true));
            }
        }
        // Check DB
        if let Some(v) =
            queries::get_value(pool, key, &ConfigScope::UserOverride, Some(uid)).await?
        {
            let mut c = cache.write().await;
            c.set(
                key,
                ConfigScope::UserOverride.as_str(),
                Some(uid),
                v.value.clone(),
            );
            return Ok((v.value, true));
        }
    }

    // 2. Try system scope
    {
        let c = cache.read().await;
        if let Some(v) = c.get(key, ConfigScope::System.as_str(), None) {
            return Ok((v, false));
        }
    }
    if let Some(v) = queries::get_value(pool, key, &ConfigScope::System, None).await? {
        let mut c = cache.write().await;
        c.set(key, ConfigScope::System.as_str(), None, v.value.clone());
        return Ok((v.value, false));
    }

    // 3. Fall back to definition default
    let def = queries::get_definition(pool, key)
        .await?
        .ok_or_else(|| ConfigError::NotFound(key.to_string()))?;
    Ok((def.default_value, false))
}

/// Reload cache TTLs from the config definitions themselves.
/// Should be called after migration to pick up seed values.
pub async fn reload_cache_ttls(
    pool: &PgPool,
    cache: &Arc<RwLock<ConfigCache>>,
) -> Result<(), ConfigError> {
    let system_ttl = get_system_value(pool, cache, "system.cache.ttlSystem").await?;
    let user_ttl = get_system_value(pool, cache, "system.cache.ttlUserOverride").await?;

    let mut c = cache.write().await;
    if let Ok(ms) = system_ttl.parse::<i64>() {
        c.system_ttl_ms = ms;
    }
    if let Ok(ms) = user_ttl.parse::<i64>() {
        c.user_override_ttl_ms = ms;
    }
    Ok(())
}
