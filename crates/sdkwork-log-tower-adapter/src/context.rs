//! Request context extraction for the tower capture middleware.

use axum::extract::ConnectInfo;
use sdkwork_log_core::RequestLogRecord;
use std::net::IpAddr;
use std::sync::Arc;

/// Extracts the request metadata the middleware needs before the inner
/// service runs. `tenant_resolver` / `path_template_resolver` /
/// `api_surface_resolver` come from the hosting application (layer
/// configuration). `trust_forwarded_headers` gates header-based client-IP
/// capture (spoof-safe default: transport extension only).
pub(crate) fn build_record_metadata(
    parts: &http::request::Parts,
    service: &Option<String>,
    tenant_resolver: &Option<Arc<crate::TenantContextResolver>>,
    path_template_resolver: &Option<Arc<crate::PathTemplateResolver>>,
    api_surface_resolver: &Option<Arc<crate::ApiSurfaceResolver>>,
    trust_forwarded_headers: bool,
) -> RequestLogRecord {
    let raw_path = parts.uri.path().to_owned();
    let path = match path_template_resolver {
        Some(resolver) => resolver(&parts.extensions, &raw_path),
        None => raw_path,
    };
    let (tenant_id, user_id, user_name) = match tenant_resolver {
        Some(resolver) => resolver(&parts.extensions),
        None => (None, None, None),
    };
    let (client_ip_hash, client_ip_masked) = match extract_client_ip(parts, trust_forwarded_headers) {
        Some(ip) => (
            Some(sdkwork_log_core::hash_client_ip(ip)),
            Some(sdkwork_log_core::mask_client_ip(ip)),
        ),
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
        user_name,
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
        client_ip_hash,
        client_ip_masked,
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

/// Extracts the client IP for one request. When `trust_forwarded_headers` is
/// true (behind a controlled reverse proxy), the first valid `x-forwarded-for`
/// entry wins, then `x-real-ip`; the `ConnectInfo<SocketAddr>` transport
/// extension is the final fallback (and the only source in the default
/// spoof-safe mode).
fn extract_client_ip(
    parts: &http::request::Parts,
    trust_forwarded_headers: bool,
) -> Option<IpAddr> {
    if trust_forwarded_headers {
        if let Some(value) = parts
            .headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .and_then(sdkwork_log_core::first_forwarded_ip)
        {
            return Some(value);
        }
        if let Some(value) = parts
            .headers
            .get("x-real-ip")
            .and_then(|value| value.to_str().ok())
            .and_then(sdkwork_log_core::parse_client_ip)
        {
            return Some(value);
        }
    }
    parts
        .extensions
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

    fn parts_with_headers(headers: &[(&str, &str)]) -> http::request::Parts {
        let mut parts = http::Request::new(())
            .into_parts()
            .0;
        parts.method = http::Method::GET;
        for (name, value) in headers {
            parts.headers.insert(
                http::HeaderName::from_bytes(name.as_bytes()).expect("header name"),
                http::HeaderValue::from_str(value).expect("header value"),
            );
        }
        parts
    }

    #[test]
    fn client_ip_ignores_forwarded_headers_by_default() {
        let parts = parts_with_headers(&[("x-forwarded-for", "1.2.3.4")]);
        assert_eq!(None, extract_client_ip(&parts, false));
    }

    #[test]
    fn client_ip_trusts_forwarded_headers_when_enabled() {
        let parts = parts_with_headers(&[("x-forwarded-for", "1.2.3.4, 10.0.0.1")]);
        assert_eq!(
            Some(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4))),
            extract_client_ip(&parts, true)
        );
        let parts = parts_with_headers(&[("x-real-ip", "2001:db8::1")]);
        assert_eq!(
            Some(IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1))),
            extract_client_ip(&parts, true)
        );
        // Malformed forwarded values are rejected, falling back to the
        // transport extension.
        let parts = parts_with_headers(&[("x-forwarded-for", "garbage")]);
        assert_eq!(None, extract_client_ip(&parts, true));
    }

    #[test]
    fn client_ip_falls_back_to_connect_info() {
        let mut parts = parts_with_headers(&[("x-forwarded-for", "1.2.3.4")]);
        parts.extensions.insert(axum::extract::ConnectInfo(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9)), 54321),
        ));
        assert_eq!(
            Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9))),
            extract_client_ip(&parts, false),
            "transport extension wins when forwarded headers are untrusted"
        );
        assert_eq!(
            Some(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4))),
            extract_client_ip(&parts, true),
            "trusted forwarded header wins over the transport extension"
        );
    }

    #[test]
    fn record_captures_masked_and_hashed_client_ip() {
        let parts = parts_with_headers(&[("x-forwarded-for", "203.0.113.7")]);
        let record = build_record_metadata(
            &parts,
            &None,
            &None,
            &None,
            &None,
            true,
        );
        assert_eq!(Some("203.0.113.x".to_owned()), record.client_ip_masked);
        let hash = record.client_ip_hash.expect("hash");
        assert_eq!(64, hash.len());
        assert!(hash.chars().all(|ch| ch.is_ascii_hexdigit()));
        // The raw address never lands in captured headers.
        assert!(!record.request_headers.as_deref().unwrap_or("").contains("203.0.113.7"));
    }
}
