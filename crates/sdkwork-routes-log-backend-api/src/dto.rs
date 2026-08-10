//! DTOs for the log backend-api (`API_SPEC.md` §13: int64-as-string on the wire).

use sdkwork_log_core::RequestLogRow;
use serde::{Deserialize, Serialize};

/// Offset list query for request logs.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AdminLogListQuery {
    /// Accepted for contract compatibility with the standard list query shape;
    /// `log_request` rows are not environment-scoped.
    pub environment: Option<String>,
    pub tenant_id: Option<String>,
    pub trace_id: Option<String>,
    pub request_id: Option<String>,
    pub api_surface: Option<String>,
    /// HTTP method filter (for example `GET`).
    pub method: Option<String>,
    pub operation_id: Option<String>,
    pub service: Option<String>,
    /// HTTP status code filter (`100..=599`).
    pub status: Option<i32>,
    /// Inclusive lower bound on `duration_ms` (milliseconds).
    pub duration_min: Option<i64>,
    /// Inclusive upper bound on `duration_ms` (milliseconds).
    pub duration_max: Option<i64>,
    /// Inclusive `created_at` lower bound (epoch seconds).
    pub created_from: Option<i64>,
    /// Inclusive `created_at` upper bound (epoch seconds).
    pub created_to: Option<i64>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    #[serde(alias = "limit")]
    pub limit: Option<i64>,
}

/// One request log row on the wire.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRequestListItem {
    pub id: String,
    pub trace_id: String,
    pub request_id: String,
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub api_surface: String,
    pub path: String,
    pub method: String,
    pub operation_id: Option<String>,
    pub service: Option<String>,
    pub environment: Option<String>,
    pub auth_mode: Option<String>,
    pub status_code: Option<i32>,
    #[serde(with = "sdkwork_utils_rust::serde_int64::option", default)]
    pub duration_ms: Option<i64>,
    pub error_code: Option<i32>,
    pub failed_stage: Option<String>,
    /// Redacted query parameters (`k=v&k=[REDACTED]`).
    pub query_params: Option<String>,
    /// Allow-listed safe request headers as a JSON object string.
    pub request_headers: Option<String>,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    pub created_at: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64::option", default)]
    pub expires_at: Option<i64>,
}

impl LogRequestListItem {
    pub fn from_row(row: RequestLogRow) -> Self {
        Self {
            id: row.id,
            trace_id: row.record.trace_id,
            request_id: row.record.request_id,
            tenant_id: row.record.tenant_id,
            user_id: row.record.user_id,
            api_surface: row.record.api_surface.as_str().to_owned(),
            path: row.record.path,
            method: row.record.method,
            operation_id: row.record.operation_id,
            service: row.record.service,
            environment: row.record.environment,
            auth_mode: row.record.auth_mode,
            status_code: row.record.status_code.map(i32::from),
            duration_ms: row.record.duration_ms.map(|value| value as i64),
            error_code: row.record.error_code,
            failed_stage: row.record.failed_stage,
            query_params: row.record.query_params,
            request_headers: row.record.request_headers,
            created_at: row.created_at,
            expires_at: row.expires_at,
        }
    }
}

/// Full request log row on the wire, including the redacted request/response
/// bodies — served only by the detail endpoint so list responses stay lean.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRequestDetailItem {
    #[serde(flatten)]
    pub summary: LogRequestListItem,
    /// Full redacted request body text (`[REDACTED]` replaces sensitive values).
    pub request_body: Option<String>,
    /// Full redacted response body text — same hygiene as `request_body`.
    pub response_body: Option<String>,
}

impl LogRequestDetailItem {
    pub fn from_row(row: RequestLogRow) -> Self {
        Self {
            request_body: row.record.request_body.clone(),
            response_body: row.record.response_body.clone(),
            summary: LogRequestListItem::from_row(row),
        }
    }
}

/// Resource envelope for the detail endpoint (`SdkWorkResourceResponse`).
#[derive(Debug, Clone, Serialize)]
pub struct LogRequestDetailEnvelope {
    pub item: LogRequestDetailItem,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_log_core::{LogApiSurface, RequestLogRecord};

    #[test]
    fn item_serializes_int64_as_string() {
        let item = LogRequestListItem::from_row(RequestLogRow {
            id: "id-1".to_owned(),
            record: RequestLogRecord {
                trace_id: "trace-1".to_owned(),
                request_id: "req-1".to_owned(),
                tenant_id: None,
                user_id: None,
                api_surface: LogApiSurface::BackendApi,
                path: "/backend/v3/api/log/request_logs".to_owned(),
                retention: None,
                method: "GET".to_owned(),
                operation_id: None,
                service: Some("svc-1".to_owned()),
                environment: Some("prod".to_owned()),
                auth_mode: Some("dual-token".to_owned()),
                status_code: Some(200),
                duration_ms: Some(42),
                error_code: None,
                failed_stage: None,
                query_params: Some("page=1".to_owned()),
                request_headers: None,
                request_body: Some("{\"prompt\":\"hi\"}".to_owned()),
                response_body: None,
            },
            created_at: 1_700_000_000,
            expires_at: Some(1_700_086_400),
        });
        let json = serde_json::to_value(&item).expect("json");
        assert_eq!("1700000000", json["createdAt"].as_str().unwrap());
        assert_eq!("42", json["durationMs"].as_str().unwrap());
        assert_eq!("200", json["statusCode"].as_i64().unwrap().to_string());
        assert_eq!("trace-1", json["traceId"].as_str().unwrap());
        assert_eq!("svc-1", json["service"].as_str().unwrap());
        assert_eq!("prod", json["environment"].as_str().unwrap());
        assert_eq!("dual-token", json["authMode"].as_str().unwrap());
        assert_eq!("page=1", json["queryParams"].as_str().unwrap());
    }
}
