// @awa-component: MCP-HookPipeline
//
//! Hook/middleware pipeline for MCP tool calls.
//!
//! Provides a trait-based hook system that runs before and after every tool
//! call. Hooks can inspect, transform, or reject calls. Built-in hooks
//! provide audit logging and access control.

pub mod access_control;
pub mod audit;

use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

/// Context passed to hooks for each tool call.
#[derive(Debug, Clone)]
pub struct HookContext {
    pub user_id: String,
    /// None for meta-tools, Some for proxied external tool calls.
    pub server_id: Option<Uuid>,
    pub tool_name: String,
    /// None for meta-tools, Some for proxied external tool calls.
    pub tool_id: Option<Uuid>,
    pub scope: HookScope,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Scope at which a hook applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookScope {
    Global,
    Server(Uuid),
    User(String),
    UserServer(String, Uuid),
}

/// Outcome of a tool call, passed to after_call hooks.
#[derive(Debug, Clone)]
pub enum ToolCallOutcome {
    Success(serde_json::Value),
    Error(String),
}

/// Errors that can occur in hooks.
#[derive(Debug, thiserror::Error)]
pub enum HookError {
    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Hook error: {0}")]
    Internal(String),
}

/// Hook trait — implement for custom hook logic.
///
/// Hooks form an ordered pipeline. `before_call` runs in order; `after_call`
/// runs in reverse order (onion model).
#[async_trait]
pub trait ToolHook: Send + Sync {
    /// Called before tool execution. Return Err to reject the call.
    async fn before_call(
        &self,
        ctx: &HookContext,
        params: &mut serde_json::Value,
    ) -> Result<(), HookError>;

    /// Called after tool execution. Can inspect or transform the outcome.
    async fn after_call(
        &self,
        ctx: &HookContext,
        outcome: &mut ToolCallOutcome,
    ) -> Result<(), HookError>;

    /// Hook identifier for debugging/logging.
    fn name(&self) -> &str;
}

/// Ordered pipeline of hooks.
///
/// `run_before` executes hooks in order, short-circuiting on error.
/// `run_after` executes hooks in reverse order (onion model).
pub struct HookPipeline {
    hooks: Vec<(HookScope, Arc<dyn ToolHook>)>,
}

impl HookPipeline {
    /// Create a new pipeline from an ordered list of scoped hooks.
    pub fn new(hooks: Vec<(HookScope, Arc<dyn ToolHook>)>) -> Self {
        Self { hooks }
    }

    /// Create an empty pipeline (no-op).
    pub fn empty() -> Self {
        Self { hooks: Vec::new() }
    }

    /// Run all before_call hooks in order. Short-circuits on error.
    pub async fn run_before(
        &self,
        ctx: &HookContext,
        params: &mut serde_json::Value,
    ) -> Result<(), HookError> {
        for (scope, hook) in &self.hooks {
            if scope_matches(scope, ctx) {
                hook.before_call(ctx, params).await?;
            }
        }
        Ok(())
    }

    /// Run all after_call hooks in reverse order.
    pub async fn run_after(
        &self,
        ctx: &HookContext,
        outcome: &mut ToolCallOutcome,
    ) -> Result<(), HookError> {
        for (scope, hook) in self.hooks.iter().rev() {
            if scope_matches(scope, ctx) {
                hook.after_call(ctx, outcome).await?;
            }
        }
        Ok(())
    }
}

/// Check whether a hook scope matches the given context.
fn scope_matches(scope: &HookScope, ctx: &HookContext) -> bool {
    match scope {
        HookScope::Global => true,
        HookScope::Server(id) => ctx.server_id.as_ref() == Some(id),
        HookScope::User(uid) => ctx.user_id == *uid,
        HookScope::UserServer(uid, sid) => {
            ctx.user_id == *uid && ctx.server_id.as_ref() == Some(sid)
        }
    }
}

/// Build the default hook pipeline with built-in hooks.
///
/// Pipeline order: AuditHook → AccessControlHook
pub fn default_pipeline(pool: sqlx::PgPool) -> HookPipeline {
    HookPipeline::new(vec![
        (
            HookScope::Global,
            Arc::new(audit::AuditHook::new(pool.clone())),
        ),
        (
            HookScope::Global,
            Arc::new(access_control::AccessControlHook::new(pool)),
        ),
    ])
}

#[cfg(test)]
mod tests;
