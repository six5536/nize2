// @awa-component: PLAN-017-ChatHandler
//
//! Chat request handler — demo stub.

use axum::Json;

use crate::error::AppResult;

/// `POST /chat` — send a chat message (demo: returns simple JSON).
pub async fn chat_handler(
    Json(_body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "content": "Hello! This is a demo response from the Nize chat endpoint.",
        "conversationId": "00000000-0000-0000-0000-000000000001",
        "messageId": "00000000-0000-0000-0000-000000000002"
    })))
}
