// @awa-component: CFG-ConfigEndpoints
//
//! Configuration request handlers.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Deserializer};

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthenticatedUser;
use crate::services::config;

// Custom deserializer that accepts number or string and converts to string
fn deserialize_number_or_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct NumberOrStringVisitor;

    impl<'de> Visitor<'de> for NumberOrStringVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a number, string, or null")
        }

        fn visit_i64<E>(self, value: i64) -> Result<Option<String>, E>
        where
            E: de::Error,
        {
            Ok(Some(value.to_string()))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Option<String>, E>
        where
            E: de::Error,
        {
            Ok(Some(value.to_string()))
        }

        fn visit_f64<E>(self, value: f64) -> Result<Option<String>, E>
        where
            E: de::Error,
        {
            Ok(Some(value.to_string()))
        }

        fn visit_str<E>(self, value: &str) -> Result<Option<String>, E>
        where
            E: de::Error,
        {
            Ok(Some(value.to_string()))
        }

        fn visit_string<E>(self, value: String) -> Result<Option<String>, E>
        where
            E: de::Error,
        {
            Ok(Some(value))
        }

        fn visit_none<E>(self) -> Result<Option<String>, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Option<String>, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_any(NumberOrStringVisitor)
}

/// User config update request — accepts number, string, or null for value.
#[derive(Debug, Deserialize)]
pub struct UpdateConfigRequest {
    #[serde(default, deserialize_with = "deserialize_number_or_string")]
    pub value: Option<String>,
}

/// Admin config update request — accepts number, string, or null for value.
#[derive(Debug, Deserialize)]
pub struct AdminUpdateConfigRequest {
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_number_or_string")]
    pub value: Option<String>,
}

/// `GET /config/user` — list all config items resolved for the authenticated user.
pub async fn user_config_list_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    let items = config::get_user_config(&state.pool, &state.config_cache, &user.0.sub).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

/// `PATCH /config/user/{key}` — update a user config override.
pub async fn user_config_update_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Path(key): Path<String>,
    Json(body): Json<UpdateConfigRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let value = body
        .value
        .ok_or_else(|| AppError::Validation("value is required".into()))?;

    let item = config::update_user_config(
        &state.pool,
        &state.config_cache,
        &user.0.sub,
        &key,
        &value,
        &state.config.mcp_encryption_key,
    )
    .await?;
    Ok(Json(serde_json::to_value(item).unwrap()))
}

/// `DELETE /config/user/{key}` — reset a user config override.
pub async fn user_config_reset_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Path(key): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    config::reset_user_config(&state.pool, &state.config_cache, &user.0.sub, &key).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// Query params for admin config listing.
#[derive(Debug, serde::Deserialize)]
pub struct AdminConfigQuery {
    pub scope: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    pub category: Option<String>,
    pub search: Option<String>,
}

/// `GET /admin/config` — list all config items (admin view).
pub async fn admin_config_list_handler(
    State(state): State<AppState>,
    Query(params): Query<AdminConfigQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = params.scope.as_deref().map(|s| match s {
        "system" => nize_core::models::config::ConfigScope::System,
        _ => nize_core::models::config::ConfigScope::UserOverride,
    });

    let items = config::get_admin_config(
        &state.pool,
        scope.as_ref(),
        params.user_id.as_deref(),
        params.category.as_deref(),
        params.search.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

/// `PATCH /admin/config/{scope}/{key}` — update a config value at a specific scope (admin).
pub async fn admin_config_update_handler(
    State(state): State<AppState>,
    Path((scope_str, key)): Path<(String, String)>,
    Json(body): Json<AdminUpdateConfigRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = match scope_str.as_str() {
        "system" => nize_core::models::config::ConfigScope::System,
        "user-override" => nize_core::models::config::ConfigScope::UserOverride,
        _ => return Err(AppError::Validation(format!("Invalid scope: {scope_str}"))),
    };

    let value = body
        .value
        .ok_or_else(|| AppError::Validation("value is required".into()))?;

    let cv = config::update_admin_config(
        &state.pool,
        &state.config_cache,
        &scope,
        &key,
        &value,
        body.user_id.as_deref(),
        &state.config.mcp_encryption_key,
    )
    .await?;
    Ok(Json(serde_json::to_value(cv).unwrap()))
}
