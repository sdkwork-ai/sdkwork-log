//! SDKWork log foundation — framework-agnostic request log domain model and store contract.
//!
//! `sdkwork-log-core` carries no web framework or database dependency so that any
//! transport (HTTP web framework, RPC, gateway, scheduled jobs) can record request
//! logs through [`RequestLogStore`] and query them uniformly.
//!
//! Correlation rules follow `OBSERVABILITY_SPEC.md`: every recorded request carries
//! the server-owned `traceId`; access-log fields must use `traceId`, never a legacy
//! `requestId`.

pub mod ip;
pub mod query;
pub mod record;
pub mod redact;
pub mod request_id;
pub mod retention;
pub mod store;

pub use ip::{first_forwarded_ip, hash_client_ip, mask_client_ip, parse_client_ip};
pub use query::{RequestLogListQuery, RequestLogPage, DEFAULT_LIST_PAGE_SIZE, MAX_LIST_PAGE_SIZE};
pub use record::{LogApiSurface, RequestLogRecord, RequestLogRow};
pub use redact::{
    capture_safe_headers, is_safe_request_header, is_sensitive_field_name, redact_body_text,
    redact_json_body, redact_query_params, redact_sensitive_value, truncate_body_text, REDACTED,
};
pub use request_id::new_request_log_id;
pub use retention::{
    DEFAULT_LOG_RETENTION_DAYS, LogRetention, LogRetentionPolicy, LogRetentionRule,
};
pub use store::{RequestLogStore, RequestLogStoreError};
