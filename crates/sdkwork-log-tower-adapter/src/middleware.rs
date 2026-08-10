//! Tower `Layer`/`Service` implementing full request logging.

use crate::capture::CaptureBody;
use crate::context::build_record_metadata;
use crate::{ApiSurfaceResolver, LogRetentionPolicy, PathTemplateResolver, StoreHandle, TenantContextResolver};
use axum::body::Body as AxumBody;
use bytes::Bytes;
use http::Request;
use http_body::Body as HttpBody;
use http_body_util::BodyExt;
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

    /// Resolves `(tenant_id, user_id)` from request extensions.
    pub fn with_tenant_resolver<F>(mut self, resolver: F) -> Self
    where
        F: Fn(&http::Extensions) -> (Option<String>, Option<String>) + Send + Sync + 'static,
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

        let body_hint = request.body().size_hint();
        let (parts, body) = request.into_parts();
        let mut record = build_record_metadata(
            &parts,
            &self.service,
            &self.tenant_resolver,
            &self.path_template_resolver,
            &self.api_surface_resolver,
        );
        if let Some(policy) = &self.retention_policy {
            record.retention = Some(policy.resolve(&record.path));
        }

        Box::pin(async move {
            // Buffer the request body only when its declared size is bounded
            // within the cap; oversized or streamed bodies are skipped so
            // memory stays flat.
            let capture_request_body = body_hint
                .upper()
                .map(|upper| upper <= max_body_bytes as u64)
                .unwrap_or(true);
            let request_body_bytes = if capture_request_body {
                match body.collect().await {
                    Ok(collected) => Some(collected.to_bytes()),
                    Err(error) => {
                        let error: Box<dyn std::error::Error + Send + Sync> = error.into();
                        warn!(%error, "request log: failed to buffer request body");
                        None
                    }
                }
            } else {
                None
            };

            if let Some(bytes) = &request_body_bytes {
                record.request_body = crate::capture_utf8_text(bytes)
                    .and_then(|text| redact_body_text(&text))
                    .map(|text| truncate_body_text(&text, max_body_bytes));
            }

            let rebuilt = Request::from_parts(
                parts,
                request_body_bytes
                    .map(AxumBody::from)
                    .unwrap_or_else(AxumBody::empty),
            );

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
