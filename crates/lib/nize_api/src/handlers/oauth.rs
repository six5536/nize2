// @zen-component: PLAN-017-OAuthHandler
//
//! OAuth callback handler — demo stub.

use axum::Json;
use axum::extract::Query;

use crate::error::AppResult;

/// Query parameters for OAuth callback.
#[derive(serde::Deserialize)]
pub struct OAuthCallbackParams {
    #[allow(dead_code)]
    pub code: Option<String>,
    #[allow(dead_code)]
    pub state: Option<String>,
}

/// `GET /auth/oauth/mcp/callback` — OAuth callback (demo).
pub async fn oauth_callback_handler(
    Query(_params): Query<OAuthCallbackParams>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "status": "ok",
        "message": "OAuth callback received (demo)"
    })))
}
