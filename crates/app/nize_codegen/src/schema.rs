//! Serde-deserializable structs matching the OpenAPI 3.0 schema subset we need.

use serde::Deserialize;
use std::collections::BTreeMap;

/// Top-level OpenAPI 3.0 document (subset).
#[derive(Debug, Deserialize)]
pub struct OpenApiDoc {
    pub info: Info,
    #[serde(default)]
    pub paths: BTreeMap<String, PathItem>,
    #[serde(default)]
    pub components: Components,
}

/// API metadata.
#[derive(Debug, Deserialize)]
pub struct Info {
    pub title: String,
    #[serde(default)]
    pub version: String,
}

/// A single path item (one URL).
#[derive(Debug, Deserialize)]
pub struct PathItem {
    #[serde(default)]
    pub get: Option<Operation>,
    #[serde(default)]
    pub post: Option<Operation>,
    #[serde(default)]
    pub put: Option<Operation>,
    #[serde(default)]
    pub delete: Option<Operation>,
    #[serde(default)]
    pub patch: Option<Operation>,
}

/// An HTTP operation.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Components section.
#[derive(Debug, Default, Deserialize)]
pub struct Components {
    #[serde(default)]
    pub schemas: BTreeMap<String, SchemaObject>,
}

/// A JSON Schema object (OpenAPI 3.0 subset).
#[derive(Debug, Deserialize)]
pub struct SchemaObject {
    #[serde(rename = "type", default)]
    pub schema_type: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub properties: BTreeMap<String, PropertyObject>,
}

/// A property within a schema.
#[derive(Debug, Deserialize)]
pub struct PropertyObject {
    #[serde(rename = "type", default)]
    pub prop_type: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub nullable: bool,
    #[serde(rename = "$ref", default)]
    pub ref_path: Option<String>,
}
