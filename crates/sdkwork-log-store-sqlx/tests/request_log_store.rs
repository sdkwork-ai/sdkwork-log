//! Integration tests for the SQLx request log store (SQLite in-memory).

use sdkwork_log_core::{
    LogApiSurface, RequestLogListQuery, RequestLogRecord, RequestLogStore,
};
use sdkwork_log_store_sqlx::SqlxRequestLogStore;
use sqlx::sqlite::SqlitePoolOptions;

async fn test_pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite memory pool");
    sqlx::migrate!("./migrations").run(&pool).await.expect("migrate");
    pool
}

fn record(trace_id: &str, request_id: &str, status: u16) -> RequestLogRecord {
    RequestLogRecord {
        trace_id: trace_id.to_owned(),
        request_id: request_id.to_owned(),
        tenant_id: Some("100001".to_owned()),
        user_id: Some("user-1".to_owned()),
        api_surface: LogApiSurface::BackendApi,
        path: "/backend/v3/api/log/request_logs".to_owned(),
        method: "GET".to_owned(),
        operation_id: Some("log.requestLogs.list".to_owned()),
        service: Some("sdkwork-api-log-assembly".to_owned()),
        environment: Some("prod".to_owned()),
        auth_mode: Some("dual-token".to_owned()),
        status_code: Some(status),
        duration_ms: Some(12),
        error_code: None,
        failed_stage: None,
        query_params: Some("page=1&page_size=20&token=[REDACTED]".to_owned()),
        request_headers: Some("{\"user-agent\":\"test-agent\"}".to_owned()),
    }
}

#[tokio::test]
async fn save_then_list_round_trip() {
    let store = SqlxRequestLogStore::new_sqlite(test_pool().await);
    store.save(record("trace-a", "req-a", 200)).await.expect("save");

    let page = store
        .list(RequestLogListQuery::default())
        .await
        .expect("list");
    assert_eq!(1, page.total);
    assert_eq!(1, page.items.len());
    let row = &page.items[0];
    assert_eq!("trace-a", row.record.trace_id);
    assert_eq!("req-a", row.record.request_id);
    assert_eq!(Some(200), row.record.status_code);
    assert_eq!(LogApiSurface::BackendApi, row.record.api_surface);
    assert!(!row.id.is_empty());
    assert!(row.expires_at.unwrap_or(0) > row.created_at);
    // OBSERVABILITY_SPEC §2 SHOULD fields survive the round trip.
    assert_eq!(Some("sdkwork-api-log-assembly".to_owned()), row.record.service);
    assert_eq!(Some("prod".to_owned()), row.record.environment);
    assert_eq!(Some("dual-token".to_owned()), row.record.auth_mode);
    assert_eq!(
        Some("page=1&page_size=20&token=[REDACTED]".to_owned()),
        row.record.query_params
    );
    assert_eq!(
        Some("{\"user-agent\":\"test-agent\"}".to_owned()),
        row.record.request_headers
    );
}

#[tokio::test]
async fn list_is_newest_first() {
    let store = SqlxRequestLogStore::new_sqlite(test_pool().await);
    store.save(record("trace-1", "req-1", 200)).await.expect("save");
    store.save(record("trace-2", "req-2", 404)).await.expect("save");

    let page = store
        .list(RequestLogListQuery::default())
        .await
        .expect("list");
    assert_eq!(2, page.items.len());
    // Same-second rows tie-break by uuid v7 id (time-ordered DESC).
    assert_eq!("req-2", page.items[0].record.request_id);
}

#[tokio::test]
async fn list_filters_by_trace_id_and_status() {
    let store = SqlxRequestLogStore::new_sqlite(test_pool().await);
    store.save(record("trace-1", "req-1", 200)).await.expect("save");
    store.save(record("trace-2", "req-2", 200)).await.expect("save");
    store.save(record("trace-3", "req-3", 500)).await.expect("save");

    let by_trace = store
        .list(
            RequestLogListQuery::default().with_trace_id("trace-2"),
        )
        .await
        .expect("list");
    assert_eq!(1, by_trace.total);
    assert_eq!("trace-2", by_trace.items[0].record.trace_id);

    let by_status = store
        .list(RequestLogListQuery::default().with_status_code(200))
        .await
        .expect("list");
    assert_eq!(2, by_status.total);
}

#[tokio::test]
async fn list_pushes_pagination_to_sql() {
    let store = SqlxRequestLogStore::new_sqlite(test_pool().await);
    for index in 0..5 {
        store
            .save(record(&format!("trace-{index}"), &format!("req-{index}"), 200))
            .await
            .expect("save");
    }

    let first = store
        .list(RequestLogListQuery::new(1, 2))
        .await
        .expect("list");
    assert_eq!(5, first.total);
    assert_eq!(2, first.items.len());

    let last = store
        .list(RequestLogListQuery::new(3, 2))
        .await
        .expect("list");
    assert_eq!(1, last.items.len());
    assert_eq!("req-0", last.items[0].record.request_id);
}

#[tokio::test]
async fn list_scopes_by_tenant() {
    let store = SqlxRequestLogStore::new_sqlite(test_pool().await);
    let mut other = record("trace-1", "req-1", 200);
    other.tenant_id = Some("100002".to_owned());
    store.save(record("trace-1", "req-0", 200)).await.expect("save");
    store.save(other).await.expect("save");

    let page = store
        .list(RequestLogListQuery::default().with_tenant_id("100001"))
        .await
        .expect("list");
    assert_eq!(1, page.total);
    assert_eq!("100001", page.items[0].record.tenant_id.as_deref().unwrap());
}

#[tokio::test]
async fn list_filters_by_service() {
    let store = SqlxRequestLogStore::new_sqlite(test_pool().await);
    let mut other = record("trace-1", "req-1", 200);
    other.service = Some("sdkwork-api-iam-assembly".to_owned());
    store.save(record("trace-1", "req-0", 200)).await.expect("save");
    store.save(other).await.expect("save");

    let page = store
        .list(RequestLogListQuery::default().with_service("sdkwork-api-iam-assembly"))
        .await
        .expect("list");
    assert_eq!(1, page.total);
    assert_eq!(
        "sdkwork-api-iam-assembly",
        page.items[0].record.service.as_deref().unwrap()
    );
}

#[tokio::test]
async fn error_code_and_ranges_are_stored() {
    let store = SqlxRequestLogStore::new_sqlite(test_pool().await);
    let mut failed = record("trace-err", "req-err", 401);
    failed.error_code = Some(40101);
    store.save(failed).await.expect("save");

    let now = sdkwork_log_store_sqlx::now_epoch_secs();
    let page = store
        .list(
            RequestLogListQuery::default()
                .with_created_range(now - 60, now + 60)
                .with_request_id("req-err"),
        )
        .await
        .expect("list");
    assert_eq!(1, page.total);
    assert_eq!(Some(40101), page.items[0].record.error_code);
    assert_eq!(Some(401), page.items[0].record.status_code);
}
