// @zen-component: PLAN-017-ConversationsHandler
//
//! Conversations request handlers.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthenticatedUser;

/// Query params for listing conversations.
#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// `GET /conversations` — list conversations for the authenticated user.
pub async fn list_conversations_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Query(params): Query<ListParams>,
) -> AppResult<Json<serde_json::Value>> {
    let user_id = parse_user_id(&user.0.sub)?;
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    let (rows, total) =
        nize_core::conversations::list_conversations(&state.pool, &user_id, limit, offset).await?;

    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "title": r.title,
                "createdAt": r.created_at.to_rfc3339(),
                "updatedAt": r.updated_at.to_rfc3339(),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "limit": limit,
        "offset": offset,
    })))
}

/// Request body for creating a conversation.
#[derive(Debug, Deserialize)]
pub struct CreateConversationBody {
    pub title: Option<String>,
}

/// `POST /conversations` — create a conversation.
pub async fn create_conversation_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Json(body): Json<CreateConversationBody>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let user_id = parse_user_id(&user.0.sub)?;
    let title = body.title.as_deref().unwrap_or("New Chat");

    let row =
        nize_core::conversations::create_conversation(&state.pool, &user_id, title).await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": row.id,
            "title": row.title,
            "createdAt": row.created_at.to_rfc3339(),
            "updatedAt": row.updated_at.to_rfc3339(),
        })),
    ))
}

/// `GET /conversations/{id}` — get a conversation with messages.
pub async fn get_conversation_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let user_id = parse_user_id(&user.0.sub)?;
    let conv_id = parse_uuid(&id)?;

    let row =
        nize_core::conversations::get_conversation(&state.pool, &user_id, &conv_id).await?;

    let message_rows =
        nize_core::conversations::get_messages(&state.pool, &conv_id).await?;

    let messages: Vec<serde_json::Value> = message_rows
        .into_iter()
        .map(|m| m.message_data)
        .collect();

    Ok(Json(serde_json::json!({
        "id": row.id,
        "title": row.title,
        "messages": messages,
        "createdAt": row.created_at.to_rfc3339(),
        "updatedAt": row.updated_at.to_rfc3339(),
    })))
}

/// `PATCH /conversations/{id}` — update a conversation (e.g., title).
pub async fn update_conversation_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Path(id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let user_id = parse_user_id(&user.0.sub)?;
    let conv_id = parse_uuid(&id)?;

    let title = body
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Validation("title is required".into()))?;

    let row =
        nize_core::conversations::update_conversation(&state.pool, &user_id, &conv_id, title)
            .await?;

    Ok(Json(serde_json::json!({
        "id": row.id,
        "title": row.title,
        "createdAt": row.created_at.to_rfc3339(),
        "updatedAt": row.updated_at.to_rfc3339(),
    })))
}

/// `DELETE /conversations/{id}` — delete a conversation and all its messages.
pub async fn delete_conversation_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let user_id = parse_user_id(&user.0.sub)?;
    let conv_id = parse_uuid(&id)?;

    let deleted =
        nize_core::conversations::delete_conversation(&state.pool, &user_id, &conv_id).await?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Conversation not found".into()))
    }
}

/// Request body for bulk saving messages.
#[derive(Debug, Deserialize)]
pub struct SaveMessagesBody {
    pub messages: Vec<serde_json::Value>,
}

/// `PUT /conversations/{id}/messages` — bulk save messages for a conversation.
pub async fn save_messages_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Path(id): Path<String>,
    Json(body): Json<SaveMessagesBody>,
) -> AppResult<StatusCode> {
    let user_id = parse_user_id(&user.0.sub)?;
    let conv_id = parse_uuid(&id)?;

    // Verify the conversation belongs to this user
    nize_core::conversations::get_conversation(&state.pool, &user_id, &conv_id).await?;

    nize_core::conversations::save_messages(&state.pool, &conv_id, &body.messages).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Parse a user ID string into a UUID.
fn parse_user_id(sub: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(sub).map_err(|_| AppError::Unauthorized("Invalid user ID".into()))
}

/// Parse a path parameter string into a UUID.
fn parse_uuid(s: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(s).map_err(|_| AppError::Validation("Invalid UUID".into()))
}
