//! Hello world endpoint — bootstrap health check.

use axum::Json;
use axum::extract::State;
use tracing::warn;

use crate::AppState;
use crate::error::AppResult;
use crate::generated::models::HelloWorldResponse;

/// `GET /hello` — verifies core lib, DB connection, and Bun sidecar.
pub async fn hello_world(State(state): State<AppState>) -> AppResult<Json<HelloWorldResponse>> {
    let greeting = nize_core::hello::hello_world();

    // Check PostgreSQL connectivity.
    let db_connected = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();

    // Check Bun sidecar availability.
    let (bun_available, bun_version) = match nize_core::bun_sidecar::check_bun_available().await {
        Ok(info) => (info.available, Some(info.version)),
        Err(e) => {
            warn!("Bun sidecar check failed: {e}");
            (false, None)
        }
    };

    Ok(Json(HelloWorldResponse {
        greeting,
        db_connected,
        bun_available,
        bun_version,
    }))
}
