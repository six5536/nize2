// @zen-component: MCP-Server
//
//! MCP server handler — defines the Nize MCP server and its tools.

use std::sync::Arc;

use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::Extension, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router,
};
use sqlx::PgPool;
use tokio::sync::RwLock;

use crate::auth::McpUser;
use crate::hooks::{HookContext, HookPipeline, HookScope, ToolCallOutcome};
use crate::tools::discovery::{
    BrowseToolDomainRequest, DiscoverToolsRequest, ExecuteToolRequest, GetToolSchemaRequest,
};
use crate::tools::hello::HelloRequest;
use crate::tools::types::{DiscoveredTool, DiscoveryResult, ServerInfo as ToolServerInfo, ToolDomain};

use nize_core::config::cache::ConfigCache;
use nize_core::mcp::execution::ClientPool;

/// Nize MCP server handler.
///
/// Holds a `PgPool` for database access, a config cache for embedding resolution,
/// a client pool for proxied tool calls, a hook pipeline, and a `ToolRouter`
/// for tool dispatch.
///
/// A new instance is created per MCP session by the `StreamableHttpService` factory.
#[derive(Clone)]
pub struct NizeMcpServer {
    pool: PgPool,
    config_cache: Arc<RwLock<ConfigCache>>,
    client_pool: Arc<ClientPool>,
    hook_pipeline: Arc<HookPipeline>,
    encryption_key: String,
    tool_router: ToolRouter<Self>,
}

/// Extract the authenticated user from rmcp request context.
///
/// The auth middleware inserts `McpUser` into HTTP request extensions;
/// rmcp injects `http::request::Parts` into tool handler context.
fn extract_user(parts: &http::request::Parts) -> Result<McpUser, ErrorData> {
    parts
        .extensions
        .get::<McpUser>()
        .cloned()
        .ok_or_else(|| {
            ErrorData::new(
                ErrorCode::INTERNAL_ERROR,
                "Missing user context — authentication may have failed".to_string(),
                None,
            )
        })
}

/// Helper to serialize a value to a pretty JSON CallToolResult.
fn json_result<T: serde::Serialize>(value: &T) -> Result<CallToolResult, ErrorData> {
    let json = serde_json::to_string_pretty(value).map_err(|e| {
        ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None)
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

/// Helper to create a hook context for meta-tools (no server_id).
fn meta_hook_ctx(user_id: &str, tool_name: &str) -> HookContext {
    HookContext {
        user_id: user_id.to_string(),
        server_id: None,
        tool_name: tool_name.to_string(),
        tool_id: None,
        scope: HookScope::Global,
        timestamp: chrono::Utc::now(),
    }
}

#[tool_router]
impl NizeMcpServer {
    /// Create a new server instance with all required dependencies.
    pub fn new(
        pool: PgPool,
        config_cache: Arc<RwLock<ConfigCache>>,
        client_pool: Arc<ClientPool>,
        hook_pipeline: Arc<HookPipeline>,
        encryption_key: String,
    ) -> Self {
        Self {
            pool,
            config_cache,
            client_pool,
            hook_pipeline,
            encryption_key,
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

    // @zen-impl: MCP-1.1_AC-1
    /// Search for tools by describing what you want to do.
    #[tool(description = "Search for tools by describing what you want to do")]
    async fn discover_tools(
        &self,
        Extension(parts): Extension<http::request::Parts>,
        Parameters(DiscoverToolsRequest { query, domain }): Parameters<DiscoverToolsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let user = extract_user(&parts)?;
        let mut params = serde_json::json!({"query": query, "domain": domain});
        let ctx = meta_hook_ctx(&user.id, "discover_tools");

        self.hook_pipeline
            .run_before(&ctx, &mut params)
            .await
            .map_err(|e| ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        let discovery_query = nize_core::mcp::discovery::DiscoveryQuery {
            query,
            domain,
            user_id: user.id.clone(),
            top_k: Some(10),
            min_similarity: Some(0.5),
        };

        let rows = nize_core::mcp::discovery::discover_tools(
            &self.pool,
            &self.config_cache,
            &discovery_query,
            &self.encryption_key,
        )
        .await
        .map_err(|e| ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        // Build response matching the existing DiscoveryResult shape
        let mut servers = std::collections::HashMap::new();
        let tools: Vec<DiscoveredTool> = rows
            .iter()
            .map(|row| {
                servers
                    .entry(row.server_id.to_string())
                    .or_insert_with(|| ToolServerInfo {
                        id: row.server_id.to_string(),
                        name: row.server_name.clone(),
                        description: row.server_description.clone(),
                    });
                DiscoveredTool {
                    id: row.tool_id.to_string(),
                    name: row.tool_name.clone(),
                    description: row.tool_description.clone(),
                    domain: row.domain.clone(),
                    server_id: row.server_id.to_string(),
                    score: row.similarity,
                }
            })
            .collect();

        let suggestion = if tools.is_empty() {
            Some("No tools matched your query. Try broader terms or list domains first.".to_string())
        } else {
            None
        };

        let result = DiscoveryResult {
            tools,
            servers,
            suggestion,
        };

        let mut outcome = ToolCallOutcome::Success(serde_json::to_value(&result).unwrap_or_default());
        let _ = self.hook_pipeline.run_after(&ctx, &mut outcome).await;

        json_result(&result)
    }

    // @zen-impl: MCP-1.2_AC-1
    /// Get detailed parameters for a specific tool.
    #[tool(description = "Get detailed parameters for a specific tool")]
    async fn get_tool_schema(
        &self,
        Extension(parts): Extension<http::request::Parts>,
        Parameters(GetToolSchemaRequest { tool_id }): Parameters<GetToolSchemaRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let user = extract_user(&parts)?;
        let mut params = serde_json::json!({"toolId": tool_id});
        let ctx = meta_hook_ctx(&user.id, "get_tool_schema");

        self.hook_pipeline
            .run_before(&ctx, &mut params)
            .await
            .map_err(|e| ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        let tool = nize_core::mcp::queries::get_tool_manifest(&self.pool, &user.id, &tool_id)
            .await
            .map_err(|e| ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?
            .ok_or_else(|| {
                ErrorData::new(
                    ErrorCode::INVALID_PARAMS,
                    format!("Tool {tool_id} not found or access denied"),
                    None,
                )
            })?;

        // Return the manifest JSONB directly — it contains the full tool schema
        let manifest = &tool.manifest;

        let mut outcome = ToolCallOutcome::Success(manifest.clone());
        let _ = self.hook_pipeline.run_after(&ctx, &mut outcome).await;

        json_result(manifest)
    }

    // @zen-impl: MCP-1.3_AC-1
    /// Run a discovered tool with parameters.
    #[tool(description = "Run a discovered tool with parameters")]
    async fn execute_tool(
        &self,
        Extension(parts): Extension<http::request::Parts>,
        Parameters(ExecuteToolRequest {
            tool_id,
            tool_name,
            params,
        }): Parameters<ExecuteToolRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let user = extract_user(&parts)?;

        let tool_uuid = uuid::Uuid::parse_str(&tool_id).map_err(|e| {
            ErrorData::new(ErrorCode::INVALID_PARAMS, format!("Invalid tool_id: {e}"), None)
        })?;

        let mut hook_params = serde_json::json!({
            "toolId": tool_id,
            "toolName": tool_name,
            "params": params,
        });
        let ctx = HookContext {
            user_id: user.id.clone(),
            server_id: None, // Will be filled after lookup
            tool_name: tool_name.clone(),
            tool_id: Some(tool_uuid),
            scope: HookScope::Global,
            timestamp: chrono::Utc::now(),
        };

        self.hook_pipeline
            .run_before(&ctx, &mut hook_params)
            .await
            .map_err(|e| ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        let exec_request = nize_core::mcp::execution::ExecutionRequest {
            tool_id: tool_uuid,
            tool_name: tool_name.clone(),
            params,
            user_id: user.id.clone(),
        };

        let result = nize_core::mcp::execution::execute_tool(
            &self.pool,
            &self.client_pool,
            &exec_request,
        )
        .await
        .map_err(|e| ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        let mut outcome = if result.success {
            ToolCallOutcome::Success(result.result.clone())
        } else {
            ToolCallOutcome::Error(format!("Tool execution failed: {}", tool_name))
        };
        let _ = self.hook_pipeline.run_after(&ctx, &mut outcome).await;

        json_result(&result)
    }

    // @zen-impl: MCP-1.4_AC-1
    /// List available tool categories.
    #[tool(description = "List available tool categories")]
    async fn list_tool_domains(
        &self,
        Extension(parts): Extension<http::request::Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let user = extract_user(&parts)?;
        let mut params = serde_json::json!({});
        let ctx = meta_hook_ctx(&user.id, "list_tool_domains");

        self.hook_pipeline
            .run_before(&ctx, &mut params)
            .await
            .map_err(|e| ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        let domain_rows = nize_core::mcp::queries::list_tool_domains(&self.pool, &user.id)
            .await
            .map_err(|e| ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        let domains: Vec<ToolDomain> = domain_rows
            .into_iter()
            .map(|row| ToolDomain {
                id: row.domain.clone(),
                name: row.domain.clone(),
                description: format!("Tools in the {} domain", row.domain),
                tool_count: row.tool_count as u32,
            })
            .collect();

        let mut outcome = ToolCallOutcome::Success(serde_json::to_value(&domains).unwrap_or_default());
        let _ = self.hook_pipeline.run_after(&ctx, &mut outcome).await;

        json_result(&domains)
    }

    // @zen-impl: MCP-1.5_AC-1
    /// List all tools in a category.
    #[tool(description = "List all tools in a category")]
    async fn browse_tool_domain(
        &self,
        Extension(parts): Extension<http::request::Parts>,
        Parameters(BrowseToolDomainRequest { domain_id }): Parameters<BrowseToolDomainRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let user = extract_user(&parts)?;
        let mut params = serde_json::json!({"domainId": domain_id});
        let ctx = meta_hook_ctx(&user.id, "browse_tool_domain");

        self.hook_pipeline
            .run_before(&ctx, &mut params)
            .await
            .map_err(|e| ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        let tool_rows = nize_core::mcp::queries::browse_tool_domain(&self.pool, &user.id, &domain_id)
            .await
            .map_err(|e| ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        let mut servers = std::collections::HashMap::new();
        let tools: Vec<DiscoveredTool> = tool_rows
            .into_iter()
            .map(|row| {
                servers
                    .entry(row.server_id.to_string())
                    .or_insert_with(|| ToolServerInfo {
                        id: row.server_id.to_string(),
                        name: row.server_name.clone(),
                        description: String::new(),
                    });
                DiscoveredTool {
                    id: row.tool_id.to_string(),
                    name: row.tool_name.clone(),
                    description: row.tool_description.clone(),
                    domain: row.domain.clone(),
                    server_id: row.server_id.to_string(),
                    score: 1.0, // Domain browsing, no similarity score
                }
            })
            .collect();

        let suggestion = if tools.is_empty() {
            Some(format!(
                "No tools found in domain \"{domain_id}\". Use list_tool_domains to see available domains."
            ))
        } else {
            None
        };

        let result = DiscoveryResult {
            tools,
            servers,
            suggestion,
        };

        let mut outcome = ToolCallOutcome::Success(serde_json::to_value(&result).unwrap_or_default());
        let _ = self.hook_pipeline.run_after(&ctx, &mut outcome).await;

        json_result(&result)
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
