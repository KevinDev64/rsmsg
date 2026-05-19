use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use server::{app_state::AppState, router::build_router};
use sqlx::postgres::PgPoolOptions;
use tower::util::ServiceExt;

fn test_app() -> axum::Router {
    let db = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://localhost/rsmsg")
        .expect("lazy pool");
    build_router(AppState { db })
}

#[tokio::test]
async fn send_message_without_auth_returns_unauthorized() {
    let app = test_app();
    let body = r#"{"message_id":"m1","from_device_uuid":"00000000-0000-0000-0000-000000000001","to_device_uuid":"00000000-0000-0000-0000-000000000002","envelope_b64":"AQ=="}"#;
    let req = Request::builder()
        .method("POST")
        .uri("/v1/send_message")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .expect("request");

    let res = app.oneshot(req).await.expect("response");
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn upload_prekeys_without_auth_returns_unauthorized() {
    let app = test_app();
    let body = r#"{"device_uuid":"00000000-0000-0000-0000-000000000001","prekeys":[]}"#;
    let req = Request::builder()
        .method("POST")
        .uri("/v1/upload_prekeys")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .expect("request");

    let res = app.oneshot(req).await.expect("response");
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn websocket_without_auth_returns_unauthorized() {
    let app = test_app();
    let req = Request::builder()
        .method("GET")
        .uri("/v1/ws")
        .header("connection", "upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(Body::empty())
        .expect("request");

    let res = app.oneshot(req).await.expect("response");
    assert_eq!(res.status(), StatusCode::UPGRADE_REQUIRED);
}
