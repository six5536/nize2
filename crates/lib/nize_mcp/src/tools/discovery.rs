// @awa-component: MCP-MetaToolHandler
//
//! Parameter types for meta-tool discovery MCP tools.

use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for the `discover_tools` meta-tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DiscoverToolsRequest {
    /// Natural language description of desired capability.
    pub query: String,
    /// Optional domain to filter results.
    pub domain: Option<String>,
}

/// Parameters for the `get_tool_schema` meta-tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetToolSchemaRequest {
    /// Tool ID from discovery results.
    pub tool_id: String,
}

/// Parameters for the `execute_tool` meta-tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExecuteToolRequest {
    /// Tool ID to execute.
    pub tool_id: String,
    /// Human-readable tool name for display.
    pub tool_name: String,
    /// Parameters matching the tool schema (JSON object). Omit or pass null for tools with no parameters.
    pub params: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Parameters for the `browse_tool_domain` meta-tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BrowseToolDomainRequest {
    /// Domain ID from `list_tool_domains`.
    pub domain_id: String,
}
