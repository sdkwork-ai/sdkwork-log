//! Request context extraction for the tower capture middleware.

use sdkwork_log_core::RequestLogRecord;
use std::sync::Arc;

/// Extracts the request metadata the middleware needs before the inner
/// service runs. `tenant_resolver` / `path_template_resolver` /
/// `api_surface_resolver` come from the hosting application (layer
/// configuration).
pub(crate) fn build_record_metadata(
    parts: &http::request::Parts,
    service: &Option<String>,
    tenant_resolver: &Option<Arc<crate::TenantContextResolver>>,
    path_template_resolver: &Option<Arc<crate::PathTemplateResolver>>,
    api_surface_resolver: &Option<Arc<crate::ApiSurfaceResolver>>,
) -> RequestLogRecord {
    let raw_path = parts.uri.path().to_owned();
    let path = match path_template_resolver {
        Some(resolver) => resolver(&parts.extensions, &raw_path),
        None => raw_path,
    };
    let (tenant_id, user_id) = match tenant_resolver {
        Some(resolver) => resolver(&parts.extensions),
        None => (None, None),
    };
    let traceparent = parts
        .headers
        .get("traceparent")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let trace_id = traceparent
        .as_deref()
        .and_then(crate::trace_id_from_traceparent)
        .or_else(|| {
            parts
                .headers
                .get("x-sdkwork-trace-id")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        });
    let trace_id = trace_id.unwrap_or_else(sdkwork_log_core::new_request_log_id);
    let request_id = parts
        .headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .unwrap_or_else(|| trace_id.clone());
    let method = parts.method.as_str().to_owned();
    let api_surface = match api_surface_resolver {
        Some(resolver) => resolver(parts.uri.path()),
        None => crate::infer_api_surface(parts.uri.path()),
    };

    let mut record = RequestLogRecord {
        trace_id,
        request_id,
        tenant_id,
        user_id,
        api_surface,
        path,
        retention: None,
        method,
        operation_id: None,
        service: service.clone(),
        environment: None,
        auth_mode: None,
        status_code: None,
        duration_ms: None,
        error_code: None,
        failed_stage: None,
        query_params: parts
            .uri
            .query()
            .and_then(|query| sdkwork_log_core::redact_query_params(Some(query))),
        request_headers: None,
        request_body: None,
        response_body: None,
    };

    // Allow-listed safe headers (credential/cookie headers never captured).
    let headers: Vec<(String, String)> = parts
        .headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|text| (name.as_str().to_owned(), text.to_owned()))
        })
        .collect();
    record.request_headers = sdkwork_log_core::capture_safe_headers(&headers);

    record
}
