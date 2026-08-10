//! Tower/axum capture middleware for the SDKWork log foundation.
//!
//! The web-framework interceptor (`sdkwork-log-web-adapter`) captures metadata
//! for services built on `sdkwork-web-framework`, but that framework streams
//! bodies and does not expose them to interceptor stages. Applications mounted
//! on tower/axum (for example `sdkwork-cloudrouter`) use this crate instead:
//! it records **one complete row per request** — metadata plus the full
//! request/response body with sensitive field values replaced by `[REDACTED]`
//! (`DATABASE_SPEC.md` §18: raw tokens, passwords, secrets, and full sensitive
//! payloads MUST NOT be stored).
//!
//! Mount the layer around the router:
//!
//! ```rust,ignore
//! use sdkwork_log_tower_adapter::RequestLoggingLayer;
//!
//! let store: Arc<dyn RequestLogStore> = Arc::new(SqlxRequestLogStore::new_postgres(pool));
//! let app = Router::new()
//!     .route("/backend/v3/api/system/records", get(records))
//!     .layer(
//!         RequestLoggingLayer::new(store)
//!             .with_service("sdkwork-cloudrouter"),
//!     );
//! ```
//!
//! The layer is self-contained: it resolves `traceId` from the `traceparent`
//! header (`x-sdkwork-trace-id` / `x-request-id` as fallbacks), infers the API
//! surface from the path prefix, and stores a best-effort row after the
//! response body completes. Hosting applications can override tenant/user
//! resolution and route-template redaction through `with_tenant_resolver` /
//! `with_path_template_resolver`.

mod capture;
mod context;
mod middleware;

pub use capture::CaptureBody;
pub use middleware::{RequestLoggingLayer, RequestLoggingMiddleware};

use sdkwork_log_core::{LogApiSurface, RequestLogStore};
use std::sync::Arc;

/// Default cap for buffered body capture. Bodies whose `size_hint` upper bound
/// exceeds this are skipped (metadata-only rows) so large uploads/downloads
/// cannot balloon memory or storage.
pub const DEFAULT_MAX_BODY_BUFFER_BYTES: usize = 256 * 1024;

/// Tenant/user context extracted from request extensions by the hosting
/// application (for example the framework principal). Returns `(tenant_id,
/// user_id)`.
pub type TenantContextResolver = dyn Fn(&http::Extensions) -> (Option<String>, Option<String>)
    + Send
    + Sync;

/// Route-template resolver: maps `(extensions, raw_path)` to the redacted
/// route template (`OBSERVABILITY_SPEC.md` §2 requires templates, never raw
/// paths). Defaults to the raw path when not supplied.
pub type PathTemplateResolver = dyn Fn(&http::Extensions, &str) -> String + Send + Sync;

/// API-surface resolver: maps a request path to its [`LogApiSurface`].
/// Defaults to [`infer_api_surface`] when not supplied — hosts with
/// non-canonical surface paths (for example open-api capability routes under
/// `/v1/...`) inject a resolver so rows are labeled correctly.
pub type ApiSurfaceResolver = dyn Fn(&str) -> LogApiSurface + Send + Sync;

pub use sdkwork_log_core::{redact_body_text, truncate_body_text, LogRetentionPolicy};

/// Infers the API surface from a path prefix (canonical framework surfaces).
pub fn infer_api_surface(path: &str) -> LogApiSurface {
    if path.starts_with("/backend/v3/api/") || path.starts_with("/backend/") {
        LogApiSurface::BackendApi
    } else if path.starts_with("/app/v3/api/") || path.starts_with("/app/") {
        LogApiSurface::AppApi
    } else if path.starts_with("/open/") || path.starts_with("/openapi") {
        LogApiSurface::OpenApi
    } else if path.starts_with("/internal/") {
        LogApiSurface::InternalApi
    } else if path.starts_with("/gateway/") {
        LogApiSurface::GatewayApi
    } else {
        LogApiSurface::Unknown
    }
}

/// Parses the trace id from a W3C `traceparent` header (`00-<32hex>-<16hex>-<2hex>`).
pub fn trace_id_from_traceparent(traceparent: &str) -> Option<String> {
    let mut parts = traceparent.trim().split('-');
    let version = parts.next()?;
    let trace_id = parts.next()?;
    let parent_id = parts.next()?;
    if version.len() == 2
        && trace_id.len() == 32
        && parent_id.len() == 16
        && parts.next().is_some()
        && trace_id.chars().all(|ch| ch.is_ascii_hexdigit())
    {
        Some(trace_id.to_ascii_lowercase())
    } else {
        None
    }
}

/// Captures a body as text when it is valid UTF-8; binary bodies are skipped
/// (`None`) — only text payloads are ever persisted.
pub fn capture_utf8_text(bytes: &[u8]) -> Option<String> {
    std::str::from_utf8(bytes).ok().map(str::to_owned)
}

/// Shared handle for `RequestLogStore` used by the middleware save task.
pub(crate) type StoreHandle = Arc<dyn RequestLogStore>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traceparent_parses_trace_id() {
        assert_eq!(
            Some("4bf92f3577b34da6a3ce929d0e0e4736".to_owned()),
            trace_id_from_traceparent(
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
            )
        );
        assert_eq!(None, trace_id_from_traceparent("00-abc-00f-01"));
        assert_eq!(None, trace_id_from_traceparent(""));
    }

    #[test]
    fn api_surface_inferred_from_prefix() {
        assert_eq!(LogApiSurface::BackendApi, infer_api_surface("/backend/v3/api/system/records"));
        assert_eq!(LogApiSurface::AppApi, infer_api_surface("/app/v3/api/chat/sessions"));
        assert_eq!(LogApiSurface::OpenApi, infer_api_surface("/open/v1/health"));
        assert_eq!(LogApiSurface::Unknown, infer_api_surface("/favicon.ico"));
    }

    #[test]
    fn binary_bodies_are_not_captured() {
        assert_eq!(None, capture_utf8_text(&[0xFF, 0x00, 0x01]));
        assert_eq!(
            Some("ok".to_owned()),
            capture_utf8_text(b"ok")
        );
    }
}
