// @zen-component: MCP-AccessControlHook
//
//! Access control hook — verifies user has access to the target MCP server.
//!
//! Checks `user_mcp_preferences` and server visibility before allowing a
//! tool call. Meta-tool calls (no server_id) are always allowed.

use async_trait::async_trait;
use sqlx::PgPool;

use super::{HookContext, HookError, ToolCallOutcome, ToolHook};

/// Access control hook: blocks calls to servers the user hasn't enabled.
pub struct AccessControlHook {
    pool: PgPool,
}

impl AccessControlHook {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

// @zen-impl: MCP-1.3_AC-1 (partial: access control for tool execution)
#[async_trait]
impl ToolHook for AccessControlHook {
    async fn before_call(
        &self,
        ctx: &HookContext,
        _params: &mut serde_json::Value,
    ) -> Result<(), HookError> {
        // Meta-tool calls (no server_id) are always allowed.
        let server_id = match ctx.server_id {
            Some(id) => id,
            None => return Ok(()),
        };

        let has_access = nize_core::mcp::queries::user_has_server_access(
            &self.pool,
            &ctx.user_id,
            &server_id.to_string(),
        )
        .await
        .map_err(|e| HookError::Internal(format!("Access check failed: {e}")))?;

        if !has_access {
            return Err(HookError::AccessDenied(format!(
                "User {} does not have access to server {}",
                ctx.user_id, server_id
            )));
        }

        Ok(())
    }

    async fn after_call(
        &self,
        _ctx: &HookContext,
        _outcome: &mut ToolCallOutcome,
    ) -> Result<(), HookError> {
        // Access control only runs before the call.
        Ok(())
    }

    fn name(&self) -> &str {
        "AccessControlHook"
    }
}
