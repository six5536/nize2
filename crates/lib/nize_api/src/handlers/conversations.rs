// @zen-component: PLAN-017-ConversationsHandler
//
//! Conversations request handlers — demo stubs.

use axum::Json;
use axum::extract::Path;
use axum::http::StatusCode;

use crate::error::AppResult;

/// `GET /conversations` — list conversations (demo).
pub async fn list_conversations_handler() -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "items": [
            {
                "id": "00000000-0000-0000-0000-000000000001",
                "title": "Demo Conversation",
                "createdAt": "2026-02-16T00:00:00Z",
                "updatedAt": "2026-02-16T00:00:00Z"
            }
        ],
        "total": 1,
        "limit": 20,
        "offset": 0
    })))
}

/// `POST /conversations` — create a conversation (demo).
pub async fn create_conversation_handler(
    Json(_body): Json<serde_json::Value>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000099",
            "title": "New Chat",
            "createdAt": "2026-02-16T00:00:00Z",
            "updatedAt": "2026-02-16T00:00:00Z"
        })),
    ))
}

/// `GET /conversations/{id}` — get a conversation with messages (demo).
pub async fn get_conversation_handler(
    Path(_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "title": "Demo Conversation",
        "messages": [
            {
                "id": "00000000-0000-0000-0000-000000000010",
                "role": "user",
                "parts": [{"type": "text", "text": "Hello"}],
                "createdAt": "2026-02-16T00:00:00Z"
            },
            {
                "id": "00000000-0000-0000-0000-000000000011",
                "role": "assistant",
                "parts": [{"type": "text", "text": "Hi there! This is a demo response."}],
                "createdAt": "2026-02-16T00:00:01Z"
            }
        ],
        "createdAt": "2026-02-16T00:00:00Z",
        "updatedAt": "2026-02-16T00:00:01Z"
    })))
}

/// `PATCH /conversations/{id}` — update a conversation (demo).
pub async fn update_conversation_handler(
    Path(_id): Path<String>,
    Json(_body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "title": "Updated Title",
        "createdAt": "2026-02-16T00:00:00Z",
        "updatedAt": "2026-02-16T00:00:02Z"
    })))
}

/// `DELETE /conversations/{id}` — delete a conversation (demo).
pub async fn delete_conversation_handler(Path(_id): Path<String>) -> StatusCode {
    StatusCode::NO_CONTENT
}
