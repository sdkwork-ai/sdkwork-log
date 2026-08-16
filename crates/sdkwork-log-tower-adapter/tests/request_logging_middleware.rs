//! Integration tests: axum app wrapped in the request logging layer persists
//! complete metadata + redacted request/response bodies.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Json;
use axum::routing::get;
use axum::Router;
use sdkwork_log_core::{RequestLogListQuery, RequestLogRecord, RequestLogRow, RequestLogStore};
use sdkwork_log_store_sqlx::SqlxRequestLogStore;
use sdkwork_log_tower_adapter::RequestLoggingLayer;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

async fn test_store() -> Option<Arc<dyn RequestLogStore>> {
    // 服务端测试必须使用 PostgreSQL（DATABASE_SPEC：authoritative-server）
    let url = std::env::var("SDKWORK_DATABASE_TEST_POSTGRES_URL").ok()?;
    let pool = sqlx::PgPool::connect(&url).await.ok()?;
    sqlx::migrate!("../sdkwork-log-store-sqlx/migrations")
        .run(&pool)
        .await
        .ok()?;
    Some(Arc::new(SqlxRequestLogStore::new_postgres(pool)))
}

fn app(store: Arc<dyn RequestLogStore>) -> Router {
    Router::new()
        .route(
            "/backend/v3/api/log/echo",
            get(|| async { Json(json!({ "result": "ok", "token": "should-be-redacted" })) })
                .post(|| async { Json(json!({ "result": "posted", "token": "should-be-redacted" })) }),
        )
        .layer(RequestLoggingLayer::new(store).with_service("tower-adapter-test"))
}

#[tokio::test]
async fn persists_full_request_and_response_with_redaction() {
    let Some(store) = test_store().await else {
        eprintln!("SKIP: SDKWORK_DATABASE_TEST_POSTGRES_URL is not configured");
        return;
    };
    let router = app(Arc::clone(&store));

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/backend/v3/api/log/echo")
                .header("content-type", "application/json")
                .header("traceparent", "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
                .header("x-request-id", "req-42")
                .body(Body::from(r#"{"prompt":"hi","apiKey":"sk-secret"}"#))
                .expect("request"),
        )
        .await
        .expect("call");

    assert_eq!(StatusCode::OK, response.status());
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response bytes");
    assert!(String::from_utf8_lossy(&body).contains("\"result\":\"posted\""));

    // Wait for the background save task.
    let page = wait_for_rows(&store, 1).await;
    let row = &page.items[0];

    assert_eq!("4bf92f3577b34da6a3ce929d0e0e4736", row.record.trace_id);
    assert_eq!("req-42", row.record.request_id);
    assert_eq!(Some(200), row.record.status_code);
    assert_eq!("tower-adapter-test", row.record.service.as_deref().unwrap());
    assert_eq!("POST", row.record.method);

    // The detail row carries the full redacted input/output.
    let detail = store
        .get_by_id(&row.id)
        .await
        .expect("get")
        .expect("row exists");
    let request_body = detail.record.request_body.as_deref().expect("request body");
    assert!(request_body.contains("\"apiKey\":\"[REDACTED]\""));
    assert!(request_body.contains("\"prompt\":\"hi\""));
    assert!(!request_body.contains("sk-secret"));

    let response_body = detail.record.response_body.as_deref().expect("response body");
    assert!(response_body.contains("\"result\":\"posted\""));
    assert!(response_body.contains("\"token\":\"[REDACTED]\""));
    assert!(!response_body.contains("should-be-redacted"));
}

#[tokio::test]
async fn list_rows_omit_bodies_and_detail_returns_them() {
    let Some(store) = test_store().await else {
        eprintln!("SKIP: SDKWORK_DATABASE_TEST_POSTGRES_URL is not configured");
        return;
    };
    let router = app(Arc::clone(&store));

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/backend/v3/api/log/echo")
                .header("x-request-id", "req-detail")
                .body(Body::from("payload"))
                .expect("request"),
        )
        .await
        .expect("call");
    // Consume the body so the capture wrapper completes and the save task runs.
    let _ = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response bytes");

    let page = wait_for_rows(&store, 1).await;
    // List projection excludes the bodies.
    assert_eq!(None, page.items[0].record.request_body);
    assert_eq!(None, page.items[0].record.response_body);

    let detail: RequestLogRow = store
        .get_by_id(&page.items[0].id)
        .await
        .expect("get")
        .expect("row exists");
    assert_eq!(Some("payload".to_owned()), detail.record.request_body);
    assert!(detail.record.response_body.is_some());
}

#[tokio::test]
async fn retention_policy_drives_expires_at() {
    let Some(store) = test_store().await else {
        eprintln!("SKIP: SDKWORK_DATABASE_TEST_POSTGRES_URL is not configured");
        return;
    };
    use sdkwork_log_core::{LogRetention, LogRetentionPolicy, LogRetentionRule};
    let policy = LogRetentionPolicy {
        default_retention: LogRetention::Days(30),
        rules: vec![LogRetentionRule {
            path_prefix: "/backend/v3/api/billing".to_owned(),
            retention: LogRetention::Permanent,
        }],
    };
    let router = Router::new()
        .route(
            "/backend/v3/api/billing/records",
            get(|| async { "ok" }),
        )
        .route(
            "/backend/v3/api/log/echo",
            get(|| async { "ok" }),
        )
        .layer(
            RequestLoggingLayer::new(Arc::clone(&store))
                .with_service("retention-test")
                .with_retention_policy(policy),
        );

    router
        .clone()
        .oneshot(Request::builder().uri("/backend/v3/api/billing/records").body(Body::empty()).expect("request"))
        .await
        .expect("call billing");
    router
        .oneshot(Request::builder().uri("/backend/v3/api/log/echo").body(Body::empty()).expect("request"))
        .await
        .expect("call default");

    let page = wait_for_rows(&store, 2).await;
    let now = sdkwork_log_store_sqlx::now_epoch_secs();
    let billing = page
        .items
        .iter()
        .find(|row| row.record.path == "/backend/v3/api/billing/records")
        .expect("billing row");
    assert_eq!(None, billing.expires_at, "permanent rows never expire");
    let default = page
        .items
        .iter()
        .find(|row| row.record.path == "/backend/v3/api/log/echo")
        .expect("default row");
    assert!(
        default.expires_at.unwrap() >= now + 30 * 86_400,
        "undeclared paths keep the 1-month default"
    );
}

async fn tenant_resolver_populates_context() {
    let Some(store) = test_store().await else {
        eprintln!("SKIP: SDKWORK_DATABASE_TEST_POSTGRES_URL is not configured");
        return;
    };
    let router = Router::new()
        .route(
            "/backend/v3/api/log/echo",
            get(|| async { "ok" }),
        )
        .layer(
            RequestLoggingLayer::new(Arc::clone(&store))
                .with_service("tenant-test")
                .with_tenant_resolver(|extensions: &http::Extensions| {
                    let _ = extensions;
                    (Some("100001".to_owned()), Some("user-9".to_owned()), Some("用户九".to_owned()))
                }),
        );

    router
        .oneshot(Request::builder().uri("/backend/v3/api/log/echo").body(Body::empty()).expect("request"))
        .await
        .expect("call");

    let page = wait_for_rows(&store, 1).await;
    assert_eq!(Some("100001".to_owned()), page.items[0].record.tenant_id);
    assert_eq!(Some("user-9".to_owned()), page.items[0].record.user_id);
    assert_eq!(Some("用户九".to_owned()), page.items[0].record.user_name);
    assert_eq!(sdkwork_log_core::LogApiSurface::BackendApi, page.items[0].record.api_surface);
}

async fn wait_for_rows(store: &Arc<dyn RequestLogStore>, expected: i64) -> sdkwork_log_core::RequestLogPage {
    for _ in 0..100 {
        let page = store.list(RequestLogListQuery::default()).await.expect("list");
        if page.total >= expected {
            return page;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("log row was not persisted in time");
}

#[allow(dead_code)]
fn _unused(_: RequestLogRecord) {}
