//! Offset-mode list query and page result (`PAGINATION_SPEC.md`).

use crate::record::{LogApiSurface, RequestLogRow};

pub const DEFAULT_LIST_PAGE_SIZE: i64 = 20;
pub const MAX_LIST_PAGE_SIZE: i64 = 200;

/// Offset-mode request log list query. Filtering, sorting, and page selection are
/// pushed to SQL (`LIMIT`/`OFFSET`) by store implementations — never collected and
/// sliced in process memory (`PAGINATION_SPEC.md` §2).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestLogListQuery {
    pub trace_id: Option<String>,
    pub request_id: Option<String>,
    pub tenant_id: Option<String>,
    pub api_surface: Option<LogApiSurface>,
    /// HTTP method filter (for example `GET`).
    pub method: Option<String>,
    pub operation_id: Option<String>,
    pub service: Option<String>,
    pub status_code: Option<u16>,
    /// Inclusive lower bound on `duration_ms` (milliseconds).
    pub duration_min: Option<i64>,
    /// Inclusive upper bound on `duration_ms` (milliseconds).
    pub duration_max: Option<i64>,
    /// Inclusive lower bound on `created_at` (epoch seconds).
    pub created_from: Option<i64>,
    /// Inclusive upper bound on `created_at` (epoch seconds).
    pub created_to: Option<i64>,
    /// 1-based page number.
    pub page: i64,
    /// Rows per page, clamped to `1..=MAX_LIST_PAGE_SIZE`.
    pub page_size: i64,
}

impl Default for RequestLogListQuery {
    fn default() -> Self {
        Self::new(1, DEFAULT_LIST_PAGE_SIZE)
    }
}

impl RequestLogListQuery {
    pub fn new(page: i64, page_size: i64) -> Self {
        Self {
            trace_id: None,
            request_id: None,
            tenant_id: None,
            api_surface: None,
            method: None,
            operation_id: None,
            service: None,
            status_code: None,
            duration_min: None,
            duration_max: None,
            created_from: None,
            created_to: None,
            page: page.max(1),
            page_size: page_size.clamp(1, MAX_LIST_PAGE_SIZE),
        }
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    pub fn with_method(mut self, method: impl Into<String>) -> Self {
        self.method = Some(method.into());
        self
    }

    pub fn with_api_surface(mut self, api_surface: LogApiSurface) -> Self {
        self.api_surface = Some(api_surface);
        self
    }

    pub fn with_operation_id(mut self, operation_id: impl Into<String>) -> Self {
        self.operation_id = Some(operation_id.into());
        self
    }

    pub fn with_service(mut self, service: impl Into<String>) -> Self {
        self.service = Some(service.into());
        self
    }

    pub fn with_status_code(mut self, status_code: u16) -> Self {
        self.status_code = Some(status_code);
        self
    }

    pub fn with_duration_min(mut self, duration_min: i64) -> Self {
        self.duration_min = Some(duration_min);
        self
    }

    pub fn with_duration_max(mut self, duration_max: i64) -> Self {
        self.duration_max = Some(duration_max);
        self
    }

    pub fn with_created_range(mut self, from: i64, to: i64) -> Self {
        self.created_from = Some(from);
        self.created_to = Some(to);
        self
    }

    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.page_size
    }
}

/// Offset-mode page result with authoritative total count.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestLogPage {
    pub items: Vec<RequestLogRow>,
    pub total: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_query_uses_spec_pagination() {
        let query = RequestLogListQuery::default();
        assert_eq!(1, query.page);
        assert_eq!(DEFAULT_LIST_PAGE_SIZE, query.page_size);
    }

    #[test]
    fn page_size_is_clamped_to_spec_bounds() {
        assert_eq!(1, RequestLogListQuery::new(0, 0).page_size);
        assert_eq!(
            MAX_LIST_PAGE_SIZE,
            RequestLogListQuery::new(1, 10_000).page_size
        );
        assert_eq!(1, RequestLogListQuery::new(0, 20).page);
    }

    #[test]
    fn offset_is_zero_based() {
        assert_eq!(0, RequestLogListQuery::new(1, 20).offset());
        assert_eq!(40, RequestLogListQuery::new(3, 20).offset());
    }
}
