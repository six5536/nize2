// @awa-component: PLAN-017-IngestHandler
//
//! Ingestion request handlers — demo stubs.

use axum::Json;
use axum::extract::Path;
use axum::http::StatusCode;

use crate::error::AppResult;

/// `POST /ingest` — upload and ingest file (demo).
pub async fn upload_handler() -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "document": {
                "id": "00000000-0000-0000-0000-000000000001",
                "filename": "demo-file.txt",
                "mimeType": "text/plain",
                "size": 1024,
                "title": "Demo File",
                "summary": "A demo document for testing.",
                "labels": ["demo", "test"],
                "category": "general",
                "createdAt": "2026-02-16T00:00:00Z",
                "updatedAt": "2026-02-16T00:00:00Z"
            },
            "chunkCount": 3
        })),
    ))
}

/// `GET /ingest` — list documents (demo).
pub async fn list_documents_handler() -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "items": [
            {
                "id": "00000000-0000-0000-0000-000000000001",
                "filename": "demo-file.txt",
                "mimeType": "text/plain",
                "size": 1024,
                "title": "Demo File",
                "summary": "A demo document for testing.",
                "labels": ["demo", "test"],
                "category": "general",
                "createdAt": "2026-02-16T00:00:00Z",
                "updatedAt": "2026-02-16T00:00:00Z"
            }
        ],
        "total": 1,
        "limit": 20,
        "offset": 0
    })))
}

/// `GET /ingest/{id}` — get document by ID (demo).
pub async fn get_document_handler(Path(_id): Path<String>) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "filename": "demo-file.txt",
        "mimeType": "text/plain",
        "size": 1024,
        "title": "Demo File",
        "summary": "A demo document for testing.",
        "labels": ["demo", "test"],
        "category": "general",
        "createdAt": "2026-02-16T00:00:00Z",
        "updatedAt": "2026-02-16T00:00:00Z"
    })))
}

/// `DELETE /ingest/{id}` — delete document (demo).
pub async fn delete_document_handler(Path(_id): Path<String>) -> StatusCode {
    StatusCode::NO_CONTENT
}
