// @awa-component: CFG-ConfigResolver
//
//! Database queries for the configuration system.

use sqlx::PgPool;

use super::ConfigError;
use crate::models::config::{ConfigDefinition, ConfigScope, ConfigValidator, ConfigValue};

/// Fetch a single config definition by key.
pub async fn get_definition(
    pool: &PgPool,
    key: &str,
) -> Result<Option<ConfigDefinition>, ConfigError> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            Option<serde_json::Value>,
            Option<serde_json::Value>,
            String,
            Option<String>,
            Option<String>,
        ),
    >(
        "SELECT key, category, type, display_type, possible_values, validators, \
         default_value, label, description \
         FROM config_definitions WHERE key = $1",
    )
    .bind(key)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(parse_definition_row))
}

/// Fetch all config definitions.
pub async fn get_all_definitions(pool: &PgPool) -> Result<Vec<ConfigDefinition>, ConfigError> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            Option<serde_json::Value>,
            Option<serde_json::Value>,
            String,
            Option<String>,
            Option<String>,
        ),
    >(
        "SELECT key, category, type, display_type, possible_values, validators, \
         default_value, label, description \
         FROM config_definitions ORDER BY category, key",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(parse_definition_row).collect())
}

/// Fetch a config value by key and scope (and optional user_id).
pub async fn get_value(
    pool: &PgPool,
    key: &str,
    scope: &ConfigScope,
    user_id: Option<&str>,
) -> Result<Option<ConfigValue>, ConfigError> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            Option<String>,
            String,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        "SELECT id::text, key, scope::text, user_id::text, value, updated_at \
         FROM config_values \
         WHERE key = $1 AND scope = $2::config_scope AND \
         (($3::text IS NULL AND user_id IS NULL) OR (user_id = $3::uuid))",
    )
    .bind(key)
    .bind(scope.as_str())
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(parse_value_row))
}

/// Fetch all config values for a given user (user-override scope).
pub async fn get_user_values(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<ConfigValue>, ConfigError> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            Option<String>,
            String,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        "SELECT id::text, key, scope::text, user_id::text, value, updated_at \
         FROM config_values \
         WHERE scope = 'user-override' AND user_id = $1::uuid",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(parse_value_row).collect())
}

/// Fetch all config values for system scope.
pub async fn get_system_values(pool: &PgPool) -> Result<Vec<ConfigValue>, ConfigError> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            Option<String>,
            String,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        "SELECT id::text, key, scope::text, user_id::text, value, updated_at \
         FROM config_values \
         WHERE scope = 'system'",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(parse_value_row).collect())
}

/// Fetch all config values (optionally filtered by scope, user_id, category).
pub async fn get_all_values(
    pool: &PgPool,
    scope: Option<&ConfigScope>,
    user_id: Option<&str>,
    category: Option<&str>,
) -> Result<Vec<ConfigValue>, ConfigError> {
    // Build query dynamically based on filters
    let mut sql = String::from(
        "SELECT cv.id::text, cv.key, cv.scope::text, cv.user_id::text, cv.value, cv.updated_at \
         FROM config_values cv",
    );
    let mut conditions: Vec<String> = Vec::new();
    let mut param_idx = 1u32;

    if category.is_some() {
        sql.push_str(" JOIN config_definitions cd ON cv.key = cd.key");
        conditions.push(format!("cd.category = ${param_idx}"));
        param_idx += 1;
    }

    if scope.is_some() {
        conditions.push(format!("cv.scope = ${param_idx}::config_scope"));
        param_idx += 1;
    }

    if user_id.is_some() {
        conditions.push(format!("cv.user_id = ${param_idx}::uuid"));
        // param_idx += 1;
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }

    sql.push_str(" ORDER BY cv.key");

    let mut query = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            Option<String>,
            String,
            chrono::DateTime<chrono::Utc>,
        ),
    >(&sql);

    // Bind parameters in the same order they were added
    if let Some(cat) = category {
        query = query.bind(cat.to_string());
    }
    if let Some(s) = scope {
        query = query.bind(s.as_str().to_string());
    }
    if let Some(uid) = user_id {
        query = query.bind(uid.to_string());
    }

    let rows = query.fetch_all(pool).await?;
    Ok(rows.into_iter().map(parse_value_row).collect())
}

/// Upsert a config value.
pub async fn upsert_value(
    pool: &PgPool,
    key: &str,
    scope: &ConfigScope,
    user_id: Option<&str>,
    value: &str,
) -> Result<ConfigValue, ConfigError> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            Option<String>,
            String,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        "INSERT INTO config_values (key, scope, user_id, value, updated_at) \
         VALUES ($1, $2::config_scope, $3::uuid, $4, now()) \
         ON CONFLICT (key, scope, user_id) \
         DO UPDATE SET value = EXCLUDED.value, updated_at = now() \
         RETURNING id::text, key, scope::text, user_id::text, value, updated_at",
    )
    .bind(key)
    .bind(scope.as_str())
    .bind(user_id)
    .bind(value)
    .fetch_one(pool)
    .await?;

    Ok(parse_value_row(row))
}

/// Delete a config value (e.g., reset user override).
pub async fn delete_value(
    pool: &PgPool,
    key: &str,
    scope: &ConfigScope,
    user_id: Option<&str>,
) -> Result<bool, ConfigError> {
    let result = sqlx::query(
        "DELETE FROM config_values \
         WHERE key = $1 AND scope = $2::config_scope AND \
         (($3::text IS NULL AND user_id IS NULL) OR (user_id = $3::uuid))",
    )
    .bind(key)
    .bind(scope.as_str())
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

// ---------------------------------------------------------------------------
// Row parsing helpers
// ---------------------------------------------------------------------------

fn parse_scope(s: &str) -> ConfigScope {
    match s {
        "system" => ConfigScope::System,
        _ => ConfigScope::UserOverride,
    }
}

#[allow(clippy::type_complexity)]
fn parse_definition_row(
    r: (
        String,
        String,
        String,
        String,
        Option<serde_json::Value>,
        Option<serde_json::Value>,
        String,
        Option<String>,
        Option<String>,
    ),
) -> ConfigDefinition {
    let possible_values: Option<Vec<String>> = r.4.and_then(|v| serde_json::from_value(v).ok());
    let validators: Option<Vec<ConfigValidator>> = r.5.and_then(|v| serde_json::from_value(v).ok());
    ConfigDefinition {
        key: r.0,
        category: r.1,
        value_type: r.2,
        display_type: r.3,
        possible_values,
        validators,
        default_value: r.6,
        label: r.7,
        description: r.8,
    }
}

fn parse_value_row(
    r: (
        String,
        String,
        String,
        Option<String>,
        String,
        chrono::DateTime<chrono::Utc>,
    ),
) -> ConfigValue {
    ConfigValue {
        id: r.0,
        key: r.1,
        scope: parse_scope(&r.2),
        user_id: r.3,
        value: r.4,
        updated_at: r.5,
    }
}
