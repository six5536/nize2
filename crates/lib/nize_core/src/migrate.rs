//! Database migration support.
//!
//! Embeds and runs SQL migrations from `nize_core/migrations/`.

use sqlx::PgPool;

/// Run all embedded database migrations against the given pool.
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
