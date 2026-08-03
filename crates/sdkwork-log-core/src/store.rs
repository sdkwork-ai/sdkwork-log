//! Framework-agnostic request log store contract.

use crate::query::{RequestLogListQuery, RequestLogPage};
use crate::record::RequestLogRecord;
use async_trait::async_trait;
use std::fmt;

/// Store failure (transport-agnostic). SQLx adapters map underlying errors here.
#[derive(Debug, Clone)]
pub struct RequestLogStoreError {
    pub message: String,
}

impl RequestLogStoreError {
    pub fn dependency(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RequestLogStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RequestLogStoreError {}

/// Framework-agnostic request log store contract: persist every HTTP request
/// (including webhook ingresses and unmatched routes) and query the persisted rows.
///
/// Store implementations assign the row `id` (UUID v7) and lifecycle timestamps
/// (`created_at`, `expires_at` TTL). `save` is synchronous from the caller's
/// perspective and fail-closed by default, mirroring the web framework audit
/// store contract (`WEB_FRAMEWORK_STANDARD.md` §9: store errors fail closed).
#[async_trait]
pub trait RequestLogStore: Send + Sync {
    /// Persists one request log record.
    async fn save(&self, record: RequestLogRecord) -> Result<(), RequestLogStoreError>;

    /// Offset-mode list with optional filters; rows newest-first
    /// (`created_at` DESC, `id` DESC). Filtering and pagination happen in SQL.
    async fn list(&self, query: RequestLogListQuery) -> Result<RequestLogPage, RequestLogStoreError>;
}
