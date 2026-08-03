//! SQLx `RequestLogStore` implementation (`log_request` table, SQLite + PostgreSQL).

use crate::pool::LogStorePool;
use crate::purge::ThrottledPurge;
use crate::now_epoch_secs;
use async_trait::async_trait;
use sdkwork_log_core::{
    new_request_log_id, LogApiSurface, RequestLogListQuery, RequestLogPage, RequestLogRecord,
    RequestLogRow, RequestLogStore, RequestLogStoreError,
};
use std::sync::Arc;

/// Default TTL for request log rows (90 days), matching the web framework audit
/// store (`WEB_FRAMEWORK_STANDARD.md`).
pub const DEFAULT_LOG_TTL_SECS: i64 = 90 * 24 * 60 * 60;

/// SQLx-backed request log store supporting SQLite and PostgreSQL.
pub struct SqlxRequestLogStore {
    pool: LogStorePool,
    ttl_secs: i64,
    purge: Arc<ThrottledPurge>,
}

impl SqlxRequestLogStore {
    /// Default TTL store for the given pool.
    pub fn new(pool: LogStorePool) -> Self {
        Self::with_ttl(pool, DEFAULT_LOG_TTL_SECS)
    }

    /// Store with an explicit TTL (epoch seconds) for `expires_at`.
    pub fn with_ttl(pool: LogStorePool, ttl_secs: i64) -> Self {
        Self {
            pool: pool.clone(),
            ttl_secs: ttl_secs.max(1),
            purge: Arc::new(ThrottledPurge::request_log(pool)),
        }
    }

    #[cfg(feature = "sqlite")]
    pub fn new_sqlite(pool: sqlx::SqlitePool) -> Self {
        Self::new(LogStorePool::Sqlite(pool))
    }

    #[cfg(feature = "postgres")]
    pub fn new_postgres(pool: sqlx::PgPool) -> Self {
        Self::new(LogStorePool::Postgres(pool))
    }
}

#[async_trait]
impl RequestLogStore for SqlxRequestLogStore {
    async fn save(&self, record: RequestLogRecord) -> Result<(), RequestLogStoreError> {
        // Throttled purge fails silently (best-effort).
        let _ = self.purge.maybe_run().await;
        let now = now_epoch_secs();
        let expires_at = now + self.ttl_secs;
        let id = new_request_log_id();
        let api_surface = record.api_surface.as_str();

        match &self.pool {
            #[cfg(feature = "sqlite")]
            LogStorePool::Sqlite(pool) => {
                sqlx::query(
                    "INSERT INTO log_request \
                     (id, trace_id, request_id, tenant_id, user_id, api_surface, path, method, \
                      operation_id, service, environment, auth_mode, status_code, duration_ms, \
                      error_code, failed_stage, query_params, request_headers, created_at, expires_at) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&id)
                .bind(&record.trace_id)
                .bind(&record.request_id)
                .bind(&record.tenant_id)
                .bind(&record.user_id)
                .bind(api_surface)
                .bind(&record.path)
                .bind(&record.method)
                .bind(&record.operation_id)
                .bind(&record.service)
                .bind(&record.environment)
                .bind(&record.auth_mode)
                .bind(record.status_code.map(i64::from))
                .bind(record.duration_ms.map(|value| value as i64))
                .bind(record.error_code.map(i64::from))
                .bind(&record.failed_stage)
                .bind(&record.query_params)
                .bind(&record.request_headers)
                .bind(now)
                .bind(expires_at)
                .execute(pool)
                .await
                .map_err(|error| {
                    RequestLogStoreError::dependency(format!("sqlite request log store: {error}"))
                })?;
            }
            #[cfg(feature = "postgres")]
            LogStorePool::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO log_request \
                     (id, trace_id, request_id, tenant_id, user_id, api_surface, path, method, \
                      operation_id, service, environment, auth_mode, status_code, duration_ms, \
                      error_code, failed_stage, query_params, request_headers, created_at, expires_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, \
                             $15, $16, $17, $18, $19, $20)",
                )
                .bind(&id)
                .bind(&record.trace_id)
                .bind(&record.request_id)
                .bind(&record.tenant_id)
                .bind(&record.user_id)
                .bind(api_surface)
                .bind(&record.path)
                .bind(&record.method)
                .bind(&record.operation_id)
                .bind(&record.service)
                .bind(&record.environment)
                .bind(&record.auth_mode)
                .bind(record.status_code.map(i64::from))
                .bind(record.duration_ms.map(|value| value as i64))
                .bind(record.error_code.map(i64::from))
                .bind(&record.failed_stage)
                .bind(&record.query_params)
                .bind(&record.request_headers)
                .bind(now)
                .bind(expires_at)
                .execute(pool)
                .await
                .map_err(|error| {
                    RequestLogStoreError::dependency(format!(
                        "postgres request log store: {error}"
                    ))
                })?;
            }
        }
        Ok(())
    }

    async fn list(&self, query: RequestLogListQuery) -> Result<RequestLogPage, RequestLogStoreError> {
        match &self.pool {
            #[cfg(feature = "sqlite")]
            LogStorePool::Sqlite(pool) => list_sqlite(pool, &query).await,
            #[cfg(feature = "postgres")]
            LogStorePool::Postgres(pool) => list_postgres(pool, &query).await,
        }
    }
}

#[derive(sqlx::FromRow)]
struct LogRow {
    id: String,
    trace_id: String,
    request_id: String,
    tenant_id: Option<String>,
    user_id: Option<String>,
    api_surface: String,
    path: String,
    method: String,
    operation_id: Option<String>,
    service: Option<String>,
    environment: Option<String>,
    auth_mode: Option<String>,
    status_code: Option<i64>,
    duration_ms: Option<i64>,
    error_code: Option<i64>,
    failed_stage: Option<String>,
    query_params: Option<String>,
    request_headers: Option<String>,
    created_at: i64,
    expires_at: Option<i64>,
}

impl LogRow {
    fn into_request_log_row(self) -> RequestLogRow {
        RequestLogRow {
            id: self.id,
            record: RequestLogRecord {
                trace_id: self.trace_id,
                request_id: self.request_id,
                tenant_id: self.tenant_id,
                user_id: self.user_id,
                api_surface: LogApiSurface::parse(&self.api_surface),
                path: self.path,
                method: self.method,
                operation_id: self.operation_id,
                service: self.service,
                environment: self.environment,
                auth_mode: self.auth_mode,
                status_code: self.status_code.and_then(|value| u16::try_from(value).ok()),
                duration_ms: self.duration_ms.and_then(|value| u64::try_from(value).ok()),
                error_code: self.error_code.and_then(|value| i32::try_from(value).ok()),
                failed_stage: self.failed_stage,
                query_params: self.query_params,
                request_headers: self.request_headers,
            },
            created_at: self.created_at,
            expires_at: self.expires_at,
        }
    }
}

const SELECT_COLUMNS: &str = "SELECT id, trace_id, request_id, tenant_id, user_id, api_surface, \
     path, method, operation_id, service, environment, auth_mode, status_code, duration_ms, \
     error_code, failed_stage, query_params, request_headers, created_at, expires_at \
     FROM log_request";

async fn list_sqlite(
    pool: &sqlx::SqlitePool,
    query: &RequestLogListQuery,
) -> Result<RequestLogPage, RequestLogStoreError> {
    let mut count = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT COUNT(*) FROM log_request WHERE 1=1",
    );
    let mut select =
        sqlx::QueryBuilder::<sqlx::Sqlite>::new(SELECT_COLUMNS.to_owned() + " WHERE 1=1");
    push_filters_sqlite(&mut count, query);
    push_filters_sqlite(&mut select, query);
    select.push(" ORDER BY created_at DESC, id DESC");
    select
        .push(" LIMIT ")
        .push_bind(query.page_size)
        .push(" OFFSET ")
        .push_bind(query.offset());

    let total: i64 = count
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|error| RequestLogStoreError::dependency(format!("sqlite request log count: {error}")))?;
    let rows: Vec<LogRow> = select
        .build_query_as()
        .fetch_all(pool)
        .await
        .map_err(|error| RequestLogStoreError::dependency(format!("sqlite request log list: {error}")))?;
    Ok(RequestLogPage {
        items: rows.into_iter().map(LogRow::into_request_log_row).collect(),
        total,
    })
}

#[cfg(feature = "postgres")]
async fn list_postgres(
    pool: &sqlx::PgPool,
    query: &RequestLogListQuery,
) -> Result<RequestLogPage, RequestLogStoreError> {
    let mut count = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT COUNT(*) FROM log_request WHERE 1=1",
    );
    let mut select =
        sqlx::QueryBuilder::<sqlx::Postgres>::new(SELECT_COLUMNS.to_owned() + " WHERE 1=1");
    push_filters_postgres(&mut count, query);
    push_filters_postgres(&mut select, query);
    select.push(" ORDER BY created_at DESC, id DESC");
    select
        .push(" LIMIT ")
        .push_bind(query.page_size)
        .push(" OFFSET ")
        .push_bind(query.offset());

    let total: i64 = count
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|error| RequestLogStoreError::dependency(format!("postgres request log count: {error}")))?;
    let rows: Vec<LogRow> = select
        .build_query_as()
        .fetch_all(pool)
        .await
        .map_err(|error| RequestLogStoreError::dependency(format!("postgres request log list: {error}")))?;
    Ok(RequestLogPage {
        items: rows.into_iter().map(LogRow::into_request_log_row).collect(),
        total,
    })
}

/// Pushes optional equality/range filters. Conditions are appended only when the
/// filter is present — a `NULL` bind would silently drop rows from comparisons.
fn push_filters_sqlite(
    qb: &mut sqlx::QueryBuilder<sqlx::Sqlite>,
    query: &RequestLogListQuery,
) {
    if let Some(value) = &query.trace_id {
        qb.push(" AND trace_id = ").push_bind(value);
    }
    if let Some(value) = &query.request_id {
        qb.push(" AND request_id = ").push_bind(value);
    }
    if let Some(value) = &query.tenant_id {
        qb.push(" AND tenant_id = ").push_bind(value);
    }
    if let Some(surface) = &query.api_surface {
        qb.push(" AND api_surface = ").push_bind(surface.as_str());
    }
    if let Some(value) = &query.operation_id {
        qb.push(" AND operation_id = ").push_bind(value);
    }
    if let Some(value) = &query.service {
        qb.push(" AND service = ").push_bind(value);
    }
    if let Some(status) = query.status_code {
        qb.push(" AND status_code = ").push_bind(i64::from(status));
    }
    if let Some(from) = query.created_from {
        qb.push(" AND created_at >= ").push_bind(from);
    }
    if let Some(to) = query.created_to {
        qb.push(" AND created_at <= ").push_bind(to);
    }
}

#[cfg(feature = "postgres")]
fn push_filters_postgres(
    qb: &mut sqlx::QueryBuilder<sqlx::Postgres>,
    query: &RequestLogListQuery,
) {
    if let Some(value) = &query.trace_id {
        qb.push(" AND trace_id = ").push_bind(value);
    }
    if let Some(value) = &query.request_id {
        qb.push(" AND request_id = ").push_bind(value);
    }
    if let Some(value) = &query.tenant_id {
        qb.push(" AND tenant_id = ").push_bind(value);
    }
    if let Some(surface) = &query.api_surface {
        qb.push(" AND api_surface = ").push_bind(surface.as_str());
    }
    if let Some(value) = &query.operation_id {
        qb.push(" AND operation_id = ").push_bind(value);
    }
    if let Some(value) = &query.service {
        qb.push(" AND service = ").push_bind(value);
    }
    if let Some(status) = query.status_code {
        qb.push(" AND status_code = ").push_bind(i64::from(status));
    }
    if let Some(from) = query.created_from {
        qb.push(" AND created_at >= ").push_bind(from);
    }
    if let Some(to) = query.created_to {
        qb.push(" AND created_at <= ").push_bind(to);
    }
}
