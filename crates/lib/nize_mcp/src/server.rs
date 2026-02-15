// @zen-component: MCP-Server
//
//! MCP server handler — defines the Nize MCP server and its tools.

use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router,
};
use sqlx::PgPool;

use crate::tools::discovery::{
    BrowseToolDomainRequest, DiscoverToolsRequest, ExecuteToolRequest, GetToolSchemaRequest,
};
use crate::tools::hello::HelloRequest;
use crate::tools::types::ExecutionResult;

/// Nize MCP server handler.
///
/// Holds a `PgPool` for database access and a `ToolRouter` for tool dispatch.
/// A new instance is created per MCP session by the `StreamableHttpService` factory.
#[derive(Clone)]
pub struct NizeMcpServer {
    #[allow(dead_code)]
    pool: PgPool,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl NizeMcpServer {
    /// Create a new server instance with a shared database pool.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            tool_router: Self::tool_router(),
        }
    }

    /// Return tool definitions registered in this server.
    #[cfg(test)]
    pub(crate) fn list_tools() -> Vec<rmcp::model::Tool> {
        Self::tool_router().list_all()
    }

    /// Say hello from Nize MCP server.
    #[tool(description = "Say hello from Nize MCP server")]
    fn hello(
        &self,
        Parameters(HelloRequest { name }): Parameters<HelloRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let greeting = format!(
            "Hello, {}! Nize MCP v{}",
            name.unwrap_or_else(|| "world".to_string()),
            nize_core::version()
        );
        Ok(CallToolResult::success(vec![Content::text(greeting)]))
    }

    // @zen-impl: MCP-1.1_AC-1 (partial: stub — returns hardcoded dummy data)
    /// Search for tools by describing what you want to do.
    #[tool(description = "Search for tools by describing what you want to do")]
    fn discover_tools(
        &self,
        Parameters(DiscoverToolsRequest { query, domain }): Parameters<DiscoverToolsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let result =
            crate::tools::dummy::discover_tools(&query, domain.as_deref());
        let json = serde_json::to_string_pretty(&result).map_err(|e| {
            ErrorData::new(rmcp::model::ErrorCode::INTERNAL_ERROR, e.to_string(), None)
        })?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // @zen-impl: MCP-1.2_AC-1 (partial: stub — returns hardcoded dummy manifest)
    /// Get detailed parameters for a specific tool.
    #[tool(description = "Get detailed parameters for a specific tool")]
    fn get_tool_schema(
        &self,
        Parameters(GetToolSchemaRequest { tool_id }): Parameters<GetToolSchemaRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let manifest = crate::tools::dummy::get_tool_manifest(&tool_id);
        let json = serde_json::to_string_pretty(&manifest).map_err(|e| {
            ErrorData::new(rmcp::model::ErrorCode::INTERNAL_ERROR, e.to_string(), None)
        })?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // @zen-impl: MCP-1.3_AC-1 (partial: stub — returns hardcoded success result)
    /// Run a discovered tool with parameters.
    #[tool(description = "Run a discovered tool with parameters")]
    fn execute_tool(
        &self,
        Parameters(ExecuteToolRequest {
            tool_id: _,
            tool_name,
            params,
        }): Parameters<ExecuteToolRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = ExecutionResult {
            success: true,
            tool_name,
            result: params,
        };
        let json = serde_json::to_string_pretty(&result).map_err(|e| {
            ErrorData::new(rmcp::model::ErrorCode::INTERNAL_ERROR, e.to_string(), None)
        })?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // @zen-impl: MCP-1.4_AC-1 (partial: stub — returns hardcoded domains)
    /// List available tool categories.
    #[tool(description = "List available tool categories")]
    fn list_tool_domains(&self) -> Result<CallToolResult, ErrorData> {
        let domains = crate::tools::dummy::list_tool_domains();
        let json = serde_json::to_string_pretty(&domains).map_err(|e| {
            ErrorData::new(rmcp::model::ErrorCode::INTERNAL_ERROR, e.to_string(), None)
        })?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // @zen-impl: MCP-1.5_AC-1 (partial: stub — returns hardcoded tools by domain)
    /// List all tools in a category.
    #[tool(description = "List all tools in a category")]
    fn browse_tool_domain(
        &self,
        Parameters(BrowseToolDomainRequest { domain_id }): Parameters<BrowseToolDomainRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::dummy::browse_tool_domain(&domain_id);
        let json = serde_json::to_string_pretty(&result).map_err(|e| {
            ErrorData::new(rmcp::model::ErrorCode::INTERNAL_ERROR, e.to_string(), None)
        })?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[tool_handler]
impl ServerHandler for NizeMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("Nize MCP server — tools for interacting with Nize".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
