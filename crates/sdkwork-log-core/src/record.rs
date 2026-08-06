//! Request log domain records (framework-agnostic).
//!
//! Field hygiene follows `OBSERVABILITY_SPEC.md` §2: routes are stored as redacted
//! templates, never raw paths; no tokens, secrets, or sensitive payloads are stored.

use serde::{Deserialize, Serialize};
use std::fmt;

/// API surface of the recorded request. Mirrors the web framework surfaces with an
/// `Other` escape hatch so non-HTTP transports (RPC, gateway) can record too.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LogApiSurface {
    #[serde(rename = "open-api")]
    OpenApi,
    #[serde(rename = "app-api")]
    AppApi,
    #[serde(rename = "backend-api")]
    BackendApi,
    #[serde(rename = "internal-api")]
    InternalApi,
    #[serde(rename = "gateway-api")]
    GatewayApi,
    #[serde(rename = "unknown")]
    Unknown,
    /// Transport surfaces not covered by the enum — raw canonical string.
    Other(String),
}

impl LogApiSurface {
    pub const OPEN_API: &'static str = "open-api";
    pub const APP_API: &'static str = "app-api";
    pub const BACKEND_API: &'static str = "backend-api";
    pub const INTERNAL_API: &'static str = "internal-api";
    pub const GATEWAY_API: &'static str = "gateway-api";
    pub const UNKNOWN: &'static str = "unknown";

    /// Canonical lowercase string form (used as the `api_surface` column value).
    pub fn as_str(&self) -> &str {
        match self {
            LogApiSurface::OpenApi => Self::OPEN_API,
            LogApiSurface::AppApi => Self::APP_API,
            LogApiSurface::BackendApi => Self::BACKEND_API,
            LogApiSurface::InternalApi => Self::INTERNAL_API,
            LogApiSurface::GatewayApi => Self::GATEWAY_API,
            LogApiSurface::Unknown => Self::UNKNOWN,
            LogApiSurface::Other(value) => value,
        }
    }

    /// Parses a canonical string form back into the enum.
    pub fn parse(value: &str) -> Self {
        match value {
            Self::OPEN_API => LogApiSurface::OpenApi,
            Self::APP_API => LogApiSurface::AppApi,
            Self::BACKEND_API => LogApiSurface::BackendApi,
            Self::INTERNAL_API => LogApiSurface::InternalApi,
            Self::GATEWAY_API => LogApiSurface::GatewayApi,
            Self::UNKNOWN => LogApiSurface::Unknown,
            _ => LogApiSurface::Other(value.to_owned()),
        }
    }
}

impl fmt::Display for LogApiSurface {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Write model: one recorded HTTP request. Persisted by a [`RequestLogStore`]
/// implementation which assigns `id` and lifecycle timestamps.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RequestLogRecord {
    /// Server-owned trace id (`x-sdkwork-trace-id` / traceparent trace id).
    /// REQUIRED — `OBSERVABILITY_SPEC.md` §2 forbids emitting legacy `requestId`
    /// in new log fields.
    pub trace_id: String,
    /// Server-owned request id, kept for correlation with audit rows.
    pub request_id: String,
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub api_surface: LogApiSurface,
    /// Redacted route template, e.g. `/app/v3/api/products/{productId}` — never
    /// raw paths with user/tenant/object identifiers.
    pub path: String,
    pub method: String,
    pub operation_id: Option<String>,
    /// Service name supplied by the wiring application
    /// (`OBSERVABILITY_SPEC.md` §2 SHOULD field: service).
    pub service: Option<String>,
    /// Deployment environment (`dev` / `test` / `prod`).
    pub environment: Option<String>,
    /// Authentication mode (`public` / `api-key` / `dual-token` / ...).
    pub auth_mode: Option<String>,
    /// HTTP response status when the row is written after the handler completes.
    pub status_code: Option<u16>,
    /// Wall-clock milliseconds from request acceptance to completion.
    pub duration_ms: Option<u64>,
    /// Numeric platform problem code for requests that failed before the handler.
    pub error_code: Option<i32>,
    /// Interceptor stage that rejected the request before the handler
    /// (`OBSERVABILITY_SPEC.md` §2 SHOULD field: interceptor stage).
    pub failed_stage: Option<String>,
    /// Redacted query parameters (`k=v&k=[REDACTED]`) — credential-shaped keys
    /// are never stored verbatim.
    pub query_params: Option<String>,
    /// Allow-listed safe request headers as a JSON object string.
    pub request_headers: Option<String>,
    /// Full request body as captured text, with sensitive field values replaced
    /// by `[REDACTED]` (`DATABASE_SPEC.md` §18: raw tokens, passwords, secrets,
    /// and full sensitive payloads MUST NOT be stored). `None` when no body was
    /// captured (empty, streaming-only, or capture disabled).
    pub request_body: Option<String>,
    /// Full response body as captured text, with the same redaction hygiene as
    /// `request_body`.
    pub response_body: Option<String>,
}

/// Read model: a persisted request log row with id and lifecycle timestamps.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RequestLogRow {
    pub id: String,
    #[serde(flatten)]
    pub record: RequestLogRecord,
    /// Epoch seconds when the row was persisted.
    pub created_at: i64,
    /// Epoch seconds TTL expiry (`None` = never expires).
    pub expires_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_surface_round_trips_canonical_strings() {
        for surface in [
            LogApiSurface::OpenApi,
            LogApiSurface::AppApi,
            LogApiSurface::BackendApi,
            LogApiSurface::InternalApi,
            LogApiSurface::GatewayApi,
            LogApiSurface::Unknown,
        ] {
            assert_eq!(surface, LogApiSurface::parse(surface.as_str()));
        }
    }

    #[test]
    fn api_surface_other_preserves_raw_value() {
        let surface = LogApiSurface::parse("rpc-internal");
        assert_eq!("rpc-internal", surface.as_str());
    }

    #[test]
    fn api_surface_serde_uses_kebab_case() {
        let json = serde_json::to_string(&LogApiSurface::BackendApi).expect("json");
        assert_eq!("\"backend-api\"", json);
    }
}
