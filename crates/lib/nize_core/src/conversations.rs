//! Conversation and message persistence.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::uuid::uuidv7;

/// Row returned by conversation queries.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConversationRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Row returned by message queries.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MessageRow {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sort_order: i32,
    pub message_data: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// List conversations for a user, ordered by most recently updated first.
pub async fn list_conversations(
    pool: &PgPool,
    user_id: &Uuid,
    limit: i64,
    offset: i64,
) -> Result<(Vec<ConversationRow>, i64), sqlx::Error> {
    let total =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM conversations WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;

    let rows = sqlx::query_as::<_, ConversationRow>(
        r#"
        SELECT id, user_id, title, created_at, updated_at
        FROM conversations
        WHERE user_id = $1
        ORDER BY updated_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok((rows, total))
}

/// Create a new conversation.
pub async fn create_conversation(
    pool: &PgPool,
    user_id: &Uuid,
    title: &str,
) -> Result<ConversationRow, sqlx::Error> {
    sqlx::query_as::<_, ConversationRow>(
        r#"
        INSERT INTO conversations (id, user_id, title)
        VALUES ($1, $2, $3)
        RETURNING id, user_id, title, created_at, updated_at
        "#,
    )
    .bind(uuidv7())
    .bind(user_id)
    .bind(title)
    .fetch_one(pool)
    .await
}

/// Get a conversation by ID (scoped to user).
pub async fn get_conversation(
    pool: &PgPool,
    user_id: &Uuid,
    conversation_id: &Uuid,
) -> Result<ConversationRow, sqlx::Error> {
    sqlx::query_as::<_, ConversationRow>(
        r#"
        SELECT id, user_id, title, created_at, updated_at
        FROM conversations
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(conversation_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Update a conversation title.
pub async fn update_conversation(
    pool: &PgPool,
    user_id: &Uuid,
    conversation_id: &Uuid,
    title: &str,
) -> Result<ConversationRow, sqlx::Error> {
    sqlx::query_as::<_, ConversationRow>(
        r#"
        UPDATE conversations
        SET title = $1, updated_at = now()
        WHERE id = $2 AND user_id = $3
        RETURNING id, user_id, title, created_at, updated_at
        "#,
    )
    .bind(title)
    .bind(conversation_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Delete a conversation (messages cascade).
pub async fn delete_conversation(
    pool: &PgPool,
    user_id: &Uuid,
    conversation_id: &Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM conversations WHERE id = $1 AND user_id = $2")
        .bind(conversation_id)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// Get messages for a conversation, ordered by sort_order.
pub async fn get_messages(
    pool: &PgPool,
    conversation_id: &Uuid,
) -> Result<Vec<MessageRow>, sqlx::Error> {
    sqlx::query_as::<_, MessageRow>(
        r#"
        SELECT id, conversation_id, sort_order, message_data, created_at
        FROM messages
        WHERE conversation_id = $1
        ORDER BY sort_order ASC
        "#,
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await
}

/// Bulk save messages — replaces all messages in a conversation.
pub async fn save_messages(
    pool: &PgPool,
    conversation_id: &Uuid,
    messages: &[serde_json::Value],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Delete existing messages
    sqlx::query("DELETE FROM messages WHERE conversation_id = $1")
        .bind(conversation_id)
        .execute(&mut *tx)
        .await?;

    // Insert new messages with sort_order
    for (i, msg) in messages.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO messages (id, conversation_id, sort_order, message_data)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(uuidv7())
        .bind(conversation_id)
        .bind(i as i32)
        .bind(msg)
        .execute(&mut *tx)
        .await?;
    }

    // Touch conversation updated_at
    sqlx::query("UPDATE conversations SET updated_at = now() WHERE id = $1")
        .bind(conversation_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}
