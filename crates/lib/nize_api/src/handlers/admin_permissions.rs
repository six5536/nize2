// @zen-component: PLAN-017-AdminPermissionsHandler
//
//! Admin permission request handlers — demo stubs.

use axum::Json;
use axum::extract::Path;
use axum::http::StatusCode;

use crate::error::AppResult;

/// `GET /admin/permissions/grants` — list all grants (demo).
pub async fn list_all_grants_handler() -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "grants": []
    })))
}

/// `DELETE /admin/permissions/grants/{grantId}` — admin revoke grant (demo).
pub async fn admin_revoke_grant_handler(Path(_grant_id): Path<String>) -> StatusCode {
    StatusCode::NO_CONTENT
}

/// `GET /admin/permissions/groups` — list all groups (demo).
pub async fn list_all_groups_handler() -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "grants": []
    })))
}

/// `GET /admin/permissions/links` — list all links (demo).
pub async fn list_all_links_handler() -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "links": []
    })))
}

/// `DELETE /admin/permissions/links/{linkId}` — admin revoke link (demo).
pub async fn admin_revoke_link_handler(Path(_link_id): Path<String>) -> StatusCode {
    StatusCode::NO_CONTENT
}

/// `PATCH /admin/permissions/users/{userId}/admin` — set admin role (demo).
pub async fn set_admin_role_handler(
    Path(_user_id): Path<String>,
    Json(_body): Json<serde_json::Value>,
) -> StatusCode {
    StatusCode::NO_CONTENT
}
