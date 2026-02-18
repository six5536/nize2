// @zen-component: MCP-AuditHook
//
//! Audit hook — logs all MCP tool calls to the `mcp_config_audit` table.
//!
//! Always first in the pipeline. Records after tool execution completes
//! (fire-and-forget — audit failures are logged but don't block the response).

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::warn;

use super::{HookContext, HookError, ToolCallOutcome, ToolHook};

/// Audit hook: records every tool call in the audit log.
pub struct AuditHook {
    pool: PgPool,
}

impl AuditHook {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

// @zen-impl: MCP-1.3_AC-1 (partial: audit logging for tool calls)
#[async_trait]
impl ToolHook for AuditHook {
    async fn before_call(
        &self,
        _ctx: &HookContext,
        _params: &mut serde_json::Value,
    ) -> Result<(), HookError> {
        // Audit runs after the call, not before.
        Ok(())
    }

    async fn after_call(
        &self,
        ctx: &HookContext,
        outcome: &mut ToolCallOutcome,
    ) -> Result<(), HookError> {
        let success = matches!(outcome, ToolCallOutcome::Success(_));
        let details = serde_json::json!({
            "toolName": ctx.tool_name,
            "toolId": ctx.tool_id.map(|id| id.to_string()),
            "success": success,
        });

        let server_id = ctx.server_id.map(|id| id.to_string());
        let server_name = ctx
            .server_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "nize-mcp".to_string());

        if let Err(e) = nize_core::mcp::queries::insert_audit_log(
            &self.pool,
            &ctx.user_id,
            server_id.as_deref(),
            &server_name,
            "tool_call",
            Some(&details),
        )
        .await
        {
            warn!("AuditHook: failed to record audit log: {e}");
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "AuditHook"
    }
}
