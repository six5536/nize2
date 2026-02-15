// @zen-component: PLAN-017-McpConfigHandler
//
//! MCP server configuration request handlers — demo stubs.

use axum::Json;
use axum::extract::Path;
use axum::http::StatusCode;

use crate::error::AppResult;

// ---------------------------------------------------------------------------
// User MCP server endpoints
// ---------------------------------------------------------------------------

/// `GET /mcp/servers` — list user MCP servers (demo).
pub async fn list_servers_handler() -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "servers": []
    })))
}

/// `POST /mcp/servers` — add user MCP server (demo).
pub async fn add_server_handler(
    Json(_body): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": "srv-demo-001",
            "name": "Demo Server",
            "url": "https://demo.example.com",
            "transport": "sse",
            "status": "active",
            "visibility": "private"
        })),
    )
}

/// `PATCH /mcp/servers/{serverId}` — update user MCP server (demo).
pub async fn update_server_handler(
    Path(_server_id): Path<String>,
    Json(_body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "id": _server_id,
        "name": "Updated Demo Server",
        "url": "https://demo.example.com",
        "transport": "sse",
        "status": "active",
        "visibility": "private"
    })))
}

/// `DELETE /mcp/servers/{serverId}` — remove user MCP server (demo).
pub async fn delete_server_handler(Path(_server_id): Path<String>) -> StatusCode {
    StatusCode::NO_CONTENT
}

/// `PATCH /mcp/servers/{serverId}/preference` — toggle server preference (demo).
pub async fn update_preference_handler(
    Path(_server_id): Path<String>,
    Json(_body): Json<serde_json::Value>,
) -> StatusCode {
    StatusCode::NO_CONTENT
}

/// `GET /mcp/servers/{serverId}/tools` — list server tools (demo).
pub async fn list_server_tools_handler(
    Path(_server_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "tools": []
    })))
}

// ---------------------------------------------------------------------------
// OAuth endpoints
// ---------------------------------------------------------------------------

/// `GET /mcp/servers/{serverId}/oauth/status` — get OAuth status (demo).
pub async fn oauth_status_handler(
    Path(_server_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "connected": false
    })))
}

/// `POST /mcp/servers/{serverId}/oauth/initiate` — initiate OAuth flow (demo).
pub async fn oauth_initiate_handler(
    Path(_server_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "authorizationUrl": "https://auth.example.com/authorize?client_id=demo"
    })))
}

/// `POST /mcp/servers/{serverId}/oauth/revoke` — revoke OAuth token (demo).
pub async fn oauth_revoke_handler(Path(_server_id): Path<String>) -> StatusCode {
    StatusCode::NO_CONTENT
}

// ---------------------------------------------------------------------------
// Test connection
// ---------------------------------------------------------------------------

/// `POST /mcp/test-connection` — test MCP server connection (demo).
pub async fn test_connection_handler(
    Json(_body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "success": true,
        "latencyMs": 42
    })))
}

// ---------------------------------------------------------------------------
// Admin MCP server endpoints
// ---------------------------------------------------------------------------

/// `GET /mcp/admin/servers` — list admin MCP servers (demo).
pub async fn admin_list_servers_handler() -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "servers": []
    })))
}

/// `POST /mcp/admin/servers` — create admin MCP server (demo).
pub async fn admin_create_server_handler(
    Json(_body): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": "srv-admin-001",
            "name": "Admin Demo Server",
            "url": "https://admin-demo.example.com",
            "transport": "sse",
            "status": "active",
            "visibility": "public"
        })),
    )
}

/// `PATCH /mcp/admin/servers/{serverId}` — update admin MCP server (demo).
pub async fn admin_update_server_handler(
    Path(_server_id): Path<String>,
    Json(_body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "id": _server_id,
        "name": "Updated Admin Demo Server",
        "url": "https://admin-demo.example.com",
        "transport": "sse",
        "status": "active",
        "visibility": "public"
    })))
}

/// `DELETE /mcp/admin/servers/{serverId}` — delete admin MCP server (demo).
pub async fn admin_delete_server_handler(Path(_server_id): Path<String>) -> StatusCode {
    StatusCode::NO_CONTENT
}
