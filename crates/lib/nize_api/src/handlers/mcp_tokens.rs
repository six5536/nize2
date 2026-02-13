//! MCP token management request handlers.

use axum::Json;
use axum::extract::{Path, State};

use crate::AppState;
use crate::error::AppResult;
use crate::generated::models::{
    CreateMcpTokenRequest, CreateMcpTokenResponse, McpTokenInfo, McpTokenListResponse,
};
use crate::middleware::auth::AuthenticatedUser;

/// `POST /auth/mcp-tokens` — create a new MCP API token.
pub async fn create_mcp_token_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Json(body): Json<CreateMcpTokenRequest>,
) -> AppResult<Json<CreateMcpTokenResponse>> {
    let (plaintext, record) =
        nize_core::auth::mcp_tokens::create_mcp_token(&state.pool, &user.0.sub, &body.name)
            .await?;
    Ok(Json(CreateMcpTokenResponse {
        id: record.id,
        token: plaintext,
        name: record.name,
        created_at: record.created_at.to_rfc3339(),
    }))
}

/// `GET /auth/mcp-tokens` — list MCP API tokens for the authenticated user.
pub async fn list_mcp_tokens_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
) -> AppResult<Json<McpTokenListResponse>> {
    let records =
        nize_core::auth::mcp_tokens::list_mcp_tokens(&state.pool, &user.0.sub).await?;
    let tokens = records
        .into_iter()
        .map(|r| McpTokenInfo {
            id: r.id,
            name: r.name,
            created_at: r.created_at.to_rfc3339(),
            expires_at: r.expires_at.map(|t| t.to_rfc3339()),
            revoked_at: r.revoked_at.map(|t| t.to_rfc3339()),
        })
        .collect();
    Ok(Json(McpTokenListResponse { tokens }))
}

/// `DELETE /auth/mcp-tokens/{id}` — revoke an MCP API token.
pub async fn revoke_mcp_token_handler(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthenticatedUser>,
    Path(token_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    nize_core::auth::mcp_tokens::revoke_mcp_token(&state.pool, &token_id).await?;
    Ok(Json(serde_json::json!({"success": true})))
}
