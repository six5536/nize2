// @awa-component: CFG-Schema
//
//! Configuration domain models.
//!
//! Types matching the `config_definitions` and `config_values` tables,
//! plus the resolved view used by the API layer.

use serde::{Deserialize, Serialize};

/// Config scope — matches the `config_scope` Postgres enum.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfigScope {
    /// Global settings, admin-controlled baseline.
    #[serde(rename = "system")]
    System,
    /// Per-user customizations that override system values.
    #[serde(rename = "user-override")]
    UserOverride,
}

impl ConfigScope {
    /// Database text representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            ConfigScope::System => "system",
            ConfigScope::UserOverride => "user-override",
        }
    }
}

impl std::fmt::Display for ConfigScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Config definition — metadata row from `config_definitions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDefinition {
    pub key: String,
    pub category: String,
    /// Data type: `"number"` | `"string"`.
    #[serde(rename = "type")]
    pub value_type: String,
    /// UI rendering hint: `"number"` | `"text"` | `"longText"` | `"selector"`.
    pub display_type: String,
    /// Enum values for selector `display_type`.
    pub possible_values: Option<Vec<String>>,
    /// Validation rules.
    pub validators: Option<Vec<ConfigValidator>>,
    /// Fallback when no value record exists.
    pub default_value: String,
    /// Human-readable name for UI.
    pub label: Option<String>,
    /// Help text for UI.
    pub description: Option<String>,
}

/// Config value — runtime row from `config_values`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValue {
    pub id: String,
    pub key: String,
    pub scope: ConfigScope,
    pub user_id: Option<String>,
    pub value: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Validator definition for config values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValidator {
    #[serde(rename = "type")]
    pub validator_type: String,
    pub value: Option<serde_json::Value>,
    pub message: Option<String>,
}

/// Resolved config item — definition metadata + resolved value + override flag.
/// This is the shape returned to API consumers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedConfigItem {
    pub key: String,
    pub category: String,
    #[serde(rename = "type")]
    pub value_type: String,
    #[serde(rename = "displayType")]
    pub display_type: String,
    #[serde(rename = "possibleValues")]
    pub possible_values: Option<Vec<String>>,
    pub validators: Option<Vec<ConfigValidator>>,
    #[serde(rename = "defaultValue")]
    pub default_value: String,
    pub label: Option<String>,
    pub description: Option<String>,
    /// The effective value (user-override → system → defaultValue).
    pub value: String,
    /// Whether the effective value comes from a user override.
    #[serde(rename = "isOverridden")]
    pub is_overridden: bool,
}

impl ResolvedConfigItem {
    /// Build from a definition and an optional resolved value.
    pub fn from_definition(
        def: &ConfigDefinition,
        value: Option<&str>,
        is_overridden: bool,
    ) -> Self {
        Self {
            key: def.key.clone(),
            category: def.category.clone(),
            value_type: def.value_type.clone(),
            display_type: def.display_type.clone(),
            possible_values: def.possible_values.clone(),
            validators: def.validators.clone(),
            default_value: def.default_value.clone(),
            label: def.label.clone(),
            description: def.description.clone(),
            value: value.unwrap_or(&def.default_value).to_string(),
            is_overridden,
        }
    }
}
