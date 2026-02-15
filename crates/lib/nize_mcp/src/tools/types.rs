// @zen-component: MCP-MetaToolTypes
//
//! Shared response types for MCP meta-tool discovery.
//!
//! These types align with the reference project's TypeScript interfaces
//! in DESIGN-MCP-semantic-tool-discovery.md.

use serde::Serialize;

/// A tool discovered via semantic search or domain browsing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredTool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    pub server_id: String,
    pub score: f64,
}

/// Metadata about an MCP server that hosts discovered tools.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// A tool category grouping related tools.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDomain {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tool_count: u32,
}

/// Result of a discovery or browse operation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryResult {
    pub tools: Vec<DiscoveredTool>,
    pub servers: std::collections::HashMap<String, ServerInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Extended tool manifest returned by `get_tool_schema`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolManifest {
    pub id: String,
    pub server_id: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    pub inputs: Vec<ToolField>,
    pub outputs: Vec<ToolField>,
    pub preconditions: Vec<String>,
    pub postconditions: Vec<String>,
    pub side_effects: Vec<String>,
}

/// A field description within a tool manifest.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Result of `execute_tool`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionResult {
    pub success: bool,
    pub tool_name: String,
    pub result: serde_json::Value,
}
