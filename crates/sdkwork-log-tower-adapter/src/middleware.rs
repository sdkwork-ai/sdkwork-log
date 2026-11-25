//! Tower `Layer`/`Service` implementing full request logging.

use crate::capture::{CaptureBody, RequestCaptureBody};
use crate::context::build_record_metadata;
use crate::{ApiSurfaceResolver, LogRetentionPolicy, PathTemplateResolver, StoreHandle, TenantContextResolver};
use axum::body::Body as AxumBody;
use bytes::Bytes;
use http::Request;
use http_body::Body as HttpBody;
use sdkwork_log_core::{redact_body_text, truncate_body_text, LogApiSurface};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Instant;
use tokio::sync::oneshot;
use tower::{Layer, Service};
use tracing::warn;

/// Default service name recorded on rows when none is configured.
const DEFAULT_SERVICE: &str = "sdkwork-log-tower-adapter";

/// Tower layer wrapping an inner service with full request logging.
#[derive(Clone)]
pub struct RequestLoggingLayer {
    store: StoreHandle,
    service: Option<String>,
    max_body_bytes: usize,
    tenant_resolver: Option<Arc<TenantContextResolver>>,
    path_template_resolver: Option<Arc<PathTemplateResolver>>,
    api_surface_resolver: Option<Arc<ApiSurfaceResolver>>,
    retention_policy: Option<Arc<LogRetentionPolicy>>,
    /// When true, `x-forwarded-for` (first valid IP) / `x-real-ip` are trusted
    /// for client-IP capture — only safe behind a controlled reverse proxy.
    trust_forwarded_headers: bool,
}

impl RequestLoggingLayer {
    /// Layer that persists one request log row per request through `store`.
    pub fn new(store: StoreHandle) -> Self {
        Self {
            store,
            service: None,
            max_body_bytes: crate::DEFAULT_MAX_BODY_BUFFER_BYTES,
            tenant_resolver: None,
            path_template_resolver: None,
            api_surface_resolver: None,
            retention_policy: None,
            trust_forwarded_headers: false,
        }
    }

    /// Sets the `service` value recorded on every row (SHOULD field per
    /// `OBSERVABILITY_SPEC.md` §2).
    pub fn with_service(mut self, service: impl Into<String>) -> Self {
        self.service = Some(service.into());
        self
    }

    /// Sets the maximum number of body bytes buffered for storage. Bodies
    /// whose `size_hint` upper bound exceeds this are skipped (metadata-only
    /// rows). Default: 256 KiB.
    pub fn with_max_body_bytes(mut self, max_body_bytes: usize) -> Self {
        self.max_body_bytes = max_body_bytes.max(1);
        self
    }

    /// Resolves `(tenant_id, user_id, user_name)` from request extensions.
    pub fn with_tenant_resolver<F>(mut self, resolver: F) -> Self
    where
        F: Fn(&http::Extensions) -> (Option<String>, Option<String>, Option<String>) + Send + Sync + 'static,
    {
        self.tenant_resolver = Some(Arc::new(resolver));
        self
    }

    /// Resolves the redacted route template from `(extensions, raw_path)`.
    pub fn with_path_template_resolver<F>(mut self, resolver: F) -> Self
    where
        F: Fn(&http::Extensions, &str) -> String + Send + Sync + 'static,
    {
        self.path_template_resolver = Some(Arc::new(resolver));
        self
    }

    /// Overrides the API-surface classification for request paths. Defaults to
    /// [`crate::infer_api_surface`]; hosts with non-canonical surface paths
    /// (for example open-api capability routes) inject their own resolver.
    pub fn with_api_surface_resolver<F>(mut self, resolver: F) -> Self
    where
        F: Fn(&str) -> LogApiSurface + Send + Sync + 'static,
    {
        self.api_surface_resolver = Some(Arc::new(resolver));
        self
    }

    /// Sets the request-log retention policy: each request's captured path is
    /// resolved against it and the row's `expires_at` follows the matched
    /// retention (`Permanent` rows are never purged; undeclared paths use the
    /// policy default of 1 month).
    pub fn with_retention_policy(mut self, policy: LogRetentionPolicy) -> Self {
        self.retention_policy = Some(Arc::new(policy));
        self
    }

    /// Controls client-IP capture. When `false` (default, spoof-safe) only the
    /// `ConnectInfo<SocketAddr>` transport extension is used. When `true`,
    /// `x-forwarded-for` (first valid IP) / `x-real-ip` are trusted and take
    /// precedence — enable only behind a controlled reverse proxy. The raw
    /// address is never persisted: rows store a SHA-256 hash plus a masked
    /// form (`DATABASE_SPEC.md` §18).
    pub fn with_trust_forwarded_headers(mut self, trust: bool) -> Self {
        self.trust_forwarded_headers = trust;
        self
    }
}

impl<S> Layer<S> for RequestLoggingLayer {
    type Service = RequestLoggingMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestLoggingMiddleware {
            inner,
            store: self.store.clone(),
            service: self.service.clone(),
            max_body_bytes: self.max_body_bytes,
            tenant_resolver: self.tenant_resolver.clone(),
            path_template_resolver: self.path_template_resolver.clone(),
            api_surface_resolver: self.api_surface_resolver.clone(),
            retention_policy: self.retention_policy.clone(),
            trust_forwarded_headers: self.trust_forwarded_headers,
        }
    }
}

/// Middleware that records one request log row (metadata + redacted bodies)
/// per request after the response body completes. Save failures are
/// best-effort: they are logged and never fail the response.
pub struct RequestLoggingMiddleware<S> {
    inner: S,
    store: StoreHandle,
    service: Option<String>,
    max_body_bytes: usize,
    tenant_resolver: Option<Arc<TenantContextResolver>>,
    path_template_resolver: Option<Arc<PathTemplateResolver>>,
    api_surface_resolver: Option<Arc<ApiSurfaceResolver>>,
    retention_policy: Option<Arc<LogRetentionPolicy>>,
    trust_forwarded_headers: bool,
}

impl<S> Clone for RequestLoggingMiddleware<S>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            store: self.store.clone(),
            service: self.service.clone(),
            max_body_bytes: self.max_body_bytes,
            tenant_resolver: self.tenant_resolver.clone(),
            path_template_resolver: self.path_template_resolver.clone(),
            api_surface_resolver: self.api_surface_resolver.clone(),
            retention_policy: self.retention_policy.clone(),
            trust_forwarded_headers: self.trust_forwarded_headers,
        }
    }
}

impl<S, B> Service<Request<B>> for RequestLoggingMiddleware<S>
where
    S: Service<Request<AxumBody>, Response = http::Response<AxumBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    B: HttpBody<Data = Bytes> + Send + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = http::Response<CaptureBody<AxumBody>>;
    type Error = std::convert::Infallible;
    type Future = Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.inner.poll_ready(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(error)) => {
                // The inner service rejected readiness; log and continue so the
                // call below surfaces a 500 with a log row.
                let error: Box<dyn std::error::Error + Send + Sync> = error.into();
                warn!(%error, "request log: inner service not ready");
                Poll::Ready(Ok(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn call(&mut self, request: Request<B>) -> Self::Future {
        let mut inner = self.inner.clone();
        let store = self.store.clone();
        let service = self
            .service
            .clone()
            .unwrap_or_else(|| DEFAULT_SERVICE.to_owned());
        let max_body_bytes = self.max_body_bytes;

        let (parts, body) = request.into_parts();
        let mut record = build_record_metadata(
            &parts,
            &self.service,
            &self.tenant_resolver,
            &self.path_template_resolver,
            &self.api_surface_resolver,
            self.trust_forwarded_headers,
        );
        if let Some(policy) = &self.retention_policy {
            record.retention = Some(policy.resolve(&record.path));
        }

        Box::pin(async move {
            // Tee the request body instead of buffering it.
            //
            // A request body can be read only once, so a capture stage that
            // consumes it MUST hand the handler the same bytes. This used to
            // buffer the body only when its declared size fit the cap and then
            // rebuild the forwarded request from that capture — so any body
            // above the cap (or one whose buffering failed) was forwarded as an
            // EMPTY body. Downstream that reads as a missing payload, not as a
            // logging problem: the CloudRouter invocation rejected every
            // `/v1/*` request larger than 256 KiB with `request body is empty`
            // while the headers still carried the real `content-length`.
            //
            // The tee forwards every frame unchanged (any size, declared or
            // streamed) and retains at most `max_body_bytes` for the log row, so
            // observation can never strand the payload.
            let captured_request_body = Arc::new(Mutex::new(Vec::<u8>::new()));
            let forwarded_body = RequestCaptureBody::new(
                body,
                Arc::clone(&captured_request_body),
                max_body_bytes,
            );
            let rebuilt = Request::from_parts(parts, AxumBody::new(forwarded_body));

            let accepted_at = Instant::now();
            let response = match inner.call(rebuilt).await {
                Ok(response) => response,
                Err(error) => {
                    let error: Box<dyn std::error::Error + Send + Sync> = error.into();
                    warn!(%error, "request log: inner service failed; recording 500");
                    http::Response::builder()
                        .status(500)
                        .body(AxumBody::empty())
                        .expect("valid 500 response")
                }
            };
            let status_code = response.status().as_u16();

            // Tee the response body; the save task fires when it completes so
            // the call future returns as soon as headers are ready.
            let captured_response = Arc::new(Mutex::new(Vec::<u8>::new()));
            let (response_tx, response_rx) = oneshot::channel();
            let (response_parts, response_body) = response.into_parts();
            let wrapped_body = CaptureBody::new(
                response_body,
                Arc::clone(&captured_response),
                max_body_bytes,
                response_tx,
            );
            let response = http::Response::from_parts(response_parts, wrapped_body);

            let mut persisted = record;
            tokio::spawn(async move {
                let response_bytes = match response_rx.await {
                    Ok(bytes) => bytes,
                    Err(_) => captured_response.lock().expect("capture lock").to_vec(),
                };
                let request_bytes = captured_request_body
                    .lock()
                    .expect("capture lock")
                    .to_vec();
                persisted.request_body = crate::capture_utf8_text(&request_bytes)
                    .and_then(|text| redact_body_text(&text))
                    .map(|text| truncate_body_text(&text, max_body_bytes));
                persisted.response_body = crate::capture_utf8_text(&response_bytes)
                    .and_then(|text| redact_body_text(&text))
                    .map(|text| truncate_body_text(&text, max_body_bytes));
                persisted.status_code = Some(status_code);
                persisted.duration_ms = Some(accepted_at.elapsed().as_millis() as u64);
                persisted.service = Some(service);
                if let Err(error) = store.save(persisted).await {
                    warn!(%error, "request log: failed to persist log row");
                }
            });

            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Request as AxumRequest;
    use axum::routing::post;
    use axum::Router;
    use sdkwork_log_core::{
        RequestLogListQuery, RequestLogPage, RequestLogRecord, RequestLogRow, RequestLogStore,
        RequestLogStoreError,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::time::Duration;
    use tower::ServiceExt;

    /// In-memory store that records every persisted row (no database needed).
    #[derive(Default)]
    struct RecordingStore {
        saved: Mutex<Vec<RequestLogRecord>>,
        saves: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl RequestLogStore for RecordingStore {
        async fn save(&self, record: RequestLogRecord) -> Result<(), RequestLogStoreError> {
            self.saves.fetch_add(1, Ordering::SeqCst);
            self.saved.lock().expect("saved lock").push(record);
            Ok(())
        }

        async fn list(
            &self,
            _query: RequestLogListQuery,
        ) -> Result<RequestLogPage, RequestLogStoreError> {
            Ok(RequestLogPage {
                items: Vec::new(),
                total: 0,
            })
        }

        async fn get_by_id(
            &self,
            _id: &str,
        ) -> Result<Option<RequestLogRow>, RequestLogStoreError> {
            Ok(None)
        }
    }

    /// Inner service that echoes the request body it actually received, so the
    /// test can prove byte-for-byte forwarding.
    fn echo_router() -> Router {
        Router::new().route(
            "/backend/v3/api/log/echo",
            post(|request: AxumRequest| async move {
                let bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
                    .await
                    .expect("inner service body");
                axum::response::Response::new(AxumBody::from(bytes))
            }),
        )
    }

    async fn call_with_cap(
        payload: Vec<u8>,
        max_body_bytes: usize,
    ) -> (Vec<u8>, Arc<RecordingStore>) {
        let store = Arc::new(RecordingStore::default());
        let app = RequestLoggingLayer::new(store.clone())
            .with_service("tower-adapter-test")
            .with_max_body_bytes(max_body_bytes)
            .layer(echo_router());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/backend/v3/api/log/echo")
                    .header("content-type", "application/json")
                    .body(AxumBody::from(payload))
                    .expect("request"),
            )
            .await
            .expect("call");
        let forwarded = axum::body::to_bytes(AxumBody::new(response.into_body()), usize::MAX)
            .await
            .expect("echoed body")
            .to_vec();
        (forwarded, store)
    }

    async fn saved_rows(store: &RecordingStore) -> Vec<RequestLogRecord> {
        for _ in 0..300 {
            if store.saves.load(Ordering::SeqCst) > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        std::mem::take(&mut *store.saved.lock().expect("saved lock"))
    }

    /// Regression: a request-log capture stage must never cost the handler its
    /// payload.
    ///
    /// The capture used to buffer the body only when its declared size fit the
    /// cap and to forward an EMPTY body otherwise. CloudRouter mounts this layer
    /// around its whole `/v1` router, so every open-api request above the 256 KiB
    /// default arrived at the invocation pipeline as an empty body — reported to
    /// callers as `400 40002 request body is empty` even though the headers still
    /// carried the real `content-length`.
    #[tokio::test]
    async fn oversized_request_body_is_forwarded_byte_for_byte() {
        let mut payload = b"{\"prompt\":\"".to_vec();
        payload.extend(std::iter::repeat(b'x').take(4096));
        payload.extend_from_slice(b"\"}");

        let (forwarded, store) = call_with_cap(payload.clone(), 256).await;

        assert_eq!(
            payload, forwarded,
            "the inner service must receive the exact request body, not an empty or truncated one"
        );
        let saved = saved_rows(store.as_ref()).await;
        assert_eq!(1, saved.len(), "one log row per request");
        let recorded = saved[0]
            .request_body
            .as_deref()
            .expect("a bounded prefix is still recorded");
        assert!(recorded.starts_with("{\"prompt\":\""));
        assert!(
            recorded.len() <= 256 + "[TRUNCATED]".len(),
            "the stored row stays bounded: recorded={} bytes",
            recorded.len()
        );
        assert!(
            recorded.len() < payload.len(),
            "an oversized body is recorded as a prefix, not in full"
        );
    }

    /// A body that fits the cap is forwarded intact and recorded in full.
    #[tokio::test]
    async fn small_request_body_is_forwarded_and_recorded_in_full() {
        let payload = br#"{"prompt":"hi"}"#.to_vec();

        let (forwarded, store) = call_with_cap(payload.clone(), 256).await;

        assert_eq!(payload, forwarded);
        let saved = saved_rows(store.as_ref()).await;
        assert_eq!(1, saved.len());
        assert_eq!(
            Some("{\"prompt\":\"hi\"}".to_owned()),
            saved[0].request_body
        );
    }
}
