//! Integration test — start ephemeral PG, build router, call /api/hello, assert response.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use nize_api::{AppState, config::ApiConfig};
use nize_core::db::LocalDbManager;
use tower::ServiceExt;

#[tokio::test]
async fn hello_endpoint_returns_expected_shape() {
    // Spin up an ephemeral PostgreSQL instance.
    let mut db = LocalDbManager::ephemeral()
        .await
        .expect("LocalDbManager::ephemeral");
    db.setup().await.expect("db setup");
    db.start().await.expect("db start");

    let pool = sqlx::PgPool::connect(&db.connection_url())
        .await
        .expect("connect to ephemeral PG");

    let state = AppState {
        pool,
        config: ApiConfig {
            bind_addr: "127.0.0.1:0".into(),
            pg_connection_url: db.connection_url(),
            jwt_secret: "test-secret".into(),
            mcp_encryption_key: "test-encryption-key".into(),
        },
        config_cache: std::sync::Arc::new(tokio::sync::RwLock::new(
            nize_core::config::cache::ConfigCache::new(),
        )),
    };

    let app = nize_api::router(state);

    let req = Request::builder()
        .uri("/api/hello")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.expect("request");

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read body");

    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse JSON");

    // Verify response shape
    assert!(json.get("greeting").is_some(), "missing 'greeting' field");
    assert!(
        json.get("dbConnected").is_some(),
        "missing 'dbConnected' field"
    );
    assert!(
        json.get("bunAvailable").is_some(),
        "missing 'bunAvailable' field"
    );

    // DB should be connected since we started an ephemeral instance.
    assert_eq!(json["dbConnected"], true, "db should be connected");

    // Greeting should contain the version.
    let greeting = json["greeting"].as_str().expect("greeting is string");
    assert!(
        greeting.starts_with("Hello from nize_core v"),
        "unexpected greeting: {greeting}"
    );

    // Bun should be available (mise provides it).
    assert_eq!(json["bunAvailable"], true, "bun should be available");
    assert!(
        json["bunVersion"].is_string(),
        "bunVersion should be a string"
    );

    // Clean up
    db.stop().await.expect("db stop");
}
