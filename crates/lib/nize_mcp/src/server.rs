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

use crate::tools::hello::HelloRequest;

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
