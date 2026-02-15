// @zen-component: PLAN-017-TraceHandler
//
//! Dev chat-trace handler — demo stub.

use axum::Json;

use crate::error::AppResult;

/// `GET /dev/chat_trace` — retrieve chat trace (demo).
pub async fn chat_trace_handler() -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "events": []
    })))
}
