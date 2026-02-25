// @awa-component: PLAN-017-PermissionsHandler
//
//! Permission request handlers — demo stubs.

use axum::Json;
use axum::extract::Path;
use axum::http::StatusCode;

use crate::error::AppResult;

/// `POST /permissions/{resourceType}/{resourceId}/grants` — create grant (demo).
pub async fn create_grant_handler(
    Path((_resource_type, _resource_id)): Path<(String, String)>,
    Json(_body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "granterId": "00000000-0000-0000-0000-000000000100",
        "granteeId": "00000000-0000-0000-0000-000000000200",
        "granteeEmail": "demo@example.com",
        "resourceType": "conversation",
        "resourceId": "00000000-0000-0000-0000-000000000300",
        "level": "view",
        "cascade": false,
        "createdAt": "2026-02-16T00:00:00Z"
    })))
}

/// `GET /permissions/{resourceType}/{resourceId}/grants` — list grants (demo).
pub async fn list_grants_handler(
    Path((_resource_type, _resource_id)): Path<(String, String)>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "grants": []
    })))
}

/// `DELETE /permissions/grants/{grantId}` — revoke grant (demo).
pub async fn revoke_grant_handler(Path(_grant_id): Path<String>) -> StatusCode {
    StatusCode::NO_CONTENT
}

/// `POST /permissions/{resourceType}/{resourceId}/links` — create share link (demo).
pub async fn create_link_handler(
    Path((_resource_type, _resource_id)): Path<(String, String)>,
    Json(_body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "ownerId": "00000000-0000-0000-0000-000000000100",
        "resourceType": "conversation",
        "resourceId": "00000000-0000-0000-0000-000000000300",
        "token": "demo-share-token",
        "level": "view",
        "cascade": false,
        "createdAt": "2026-02-16T00:00:00Z",
        "url": "https://nize.local/permissions/shared/demo-share-token"
    })))
}

/// `GET /permissions/{resourceType}/{resourceId}/links` — list share links (demo).
pub async fn list_links_handler(
    Path((_resource_type, _resource_id)): Path<(String, String)>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "links": []
    })))
}

/// `DELETE /permissions/links/{linkId}` — revoke share link (demo).
pub async fn revoke_link_handler(Path(_link_id): Path<String>) -> StatusCode {
    StatusCode::NO_CONTENT
}

/// `GET /permissions/shared/{token}` — access shared resource (demo).
pub async fn access_shared_handler(
    Path(_token): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "resourceType": "conversation",
        "resourceId": "00000000-0000-0000-0000-000000000300",
        "level": "view",
        "cascade": false
    })))
}
