//! Web framework capture adapter for the SDKWork log foundation.
//!
//! [`RequestLoggingInterceptor`] implements `sdkwork-web-framework`'s
//! `WebCallInterceptor` (EP-14 extension point) and is appended to the standard
//! interceptor chain **without changing the 18-stage order**. Because every HTTP
//! request — all API surfaces, webhook ingresses, WebSocket upgrades, and
//! contract fallback — passes through the framework layer, one interceptor covers
//! them all and persists a `traceId`-carrying request log row per request
//! (`OBSERVABILITY_SPEC.md` §2: access logs must use `traceId`).
//!
//! The interceptor captures, in `before`, the redacted query parameters and
//! allow-listed safe request headers into [`WebCallState`], then persists the
//! full record in `after` (status, duration, error code, failed stage, auth
//! mode, environment, service).
//!
//! Wiring example (in the application's standalone-gateway assembly):
//!
//! ```rust,ignore
//! let store: Arc<dyn RequestLogStore> = Arc::new(SqlxRequestLogStore::new_sqlite(pool));
//! let chain = WebCallInterceptorChain::standard()
//!     .with_interceptor(
//!         RequestLoggingInterceptor::new(store).with_service("sdkwork-api-iam-assembly"),
//!     );
//! let framework = WebFramework::builder(resolver)
//!     .route_manifest(manifest)
//!     .call_chain(chain)
//!     .build();
//! ```

use async_trait::async_trait;
use sdkwork_log_core::{
    capture_safe_headers, redact_query_params, LogApiSurface, RequestLogRecord, RequestLogStore,
};
use sdkwork_web_core::{
    problem::redact_path_template, trace::trace_id_from_traceparent, WebAuthMode, WebCallInterceptor,
    WebCallRuntime, WebCallStage, WebCallState, WebEnvironment, WebFrameworkError,
    WebRequestContextResolver,
};
use std::marker::PhantomData;
use std::sync::Arc;

/// Persists one request-log row per HTTP request after the handler completes.
///
/// Store failures are fail-closed (`dependency_unavailable`), matching the web
/// framework audit store contract (`WEB_FRAMEWORK_STANDARD.md` §9).
pub struct RequestLoggingInterceptor<R>
where
    R: WebRequestContextResolver + Clone,
{
    store: Arc<dyn RequestLogStore>,
    service: Option<String>,
    _runtime: PhantomData<R>,
}

impl<R> RequestLoggingInterceptor<R>
where
    R: WebRequestContextResolver + Clone,
{
    pub fn new(store: Arc<dyn RequestLogStore>) -> Self {
        Self {
            store,
            service: None,
            _runtime: PhantomData,
        }
    }

    /// Sets the service name recorded on every row (SHOULD field per
    /// `OBSERVABILITY_SPEC.md` §2).
    pub fn with_service(mut self, service: impl Into<String>) -> Self {
        self.service = Some(service.into());
        self
    }
}

#[async_trait]
impl<R> WebCallInterceptor<R> for RequestLoggingInterceptor<R>
where
    R: WebRequestContextResolver + Clone,
{
    fn name(&self) -> &'static str {
        "request_logging"
    }

    fn stage(&self) -> WebCallStage {
        WebCallStage::Audit
    }

    async fn before(
        &self,
        state: &mut WebCallState,
        request: &mut axum::extract::Request,
        _runtime: &WebCallRuntime<R>,
    ) -> Result<(), WebFrameworkError> {
        // Captured values are redacted/allow-listed at capture time
        // (`OBSERVABILITY_SPEC.md` §2: no tokens, secrets, or sensitive payloads).
        state.redacted_query = redact_query_params(request.uri().query());
        let headers: Vec<(String, String)> = request
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_owned(), value.to_owned()))
            })
            .collect();
        state.safe_request_headers = capture_safe_headers(&headers);
        Ok(())
    }

    async fn after(
        &self,
        state: &WebCallState,
        response: &mut axum::response::Response,
        runtime: &WebCallRuntime<R>,
    ) -> Result<(), WebFrameworkError> {
        let record = request_log_record_from_state(
            state,
            response.status().as_u16(),
            self.service.as_deref(),
            web_environment_str(&runtime.profile.environment),
        );
        self.store.save(record).await.map_err(|error| {
            tracing::warn!(
                request_id = ?state.request_id_value(),
                error = %error,
                "request log store write failed"
            );
            WebFrameworkError::dependency_unavailable(format!(
                "request log store error: {}",
                error.message
            ))
        })?;
        Ok(())
    }
}

/// Convenience: appends [`RequestLoggingInterceptor`] to a standard chain.
pub fn with_request_logging<R>(
    chain: sdkwork_web_core::WebCallInterceptorChain<R>,
    store: Arc<dyn RequestLogStore>,
) -> sdkwork_web_core::WebCallInterceptorChain<R>
where
    R: WebRequestContextResolver + Clone,
{
    chain.with_interceptor(RequestLoggingInterceptor::new(store))
}

/// Canonical `WebEnvironment` string form (`dev` / `test` / `prod`).
fn web_environment_str(environment: &WebEnvironment) -> Option<&'static str> {
    match environment {
        WebEnvironment::Dev => Some("dev"),
        WebEnvironment::Test => Some("test"),
        WebEnvironment::Prod => Some("prod"),
    }
}

/// Canonical `WebAuthMode` string form.
fn web_auth_mode_str(auth_mode: &WebAuthMode) -> Option<&'static str> {
    match auth_mode {
        WebAuthMode::Public => Some("public"),
        WebAuthMode::BootstrapBody => Some("bootstrap-body"),
        WebAuthMode::CredentialEntryBootstrap => Some("credential-entry-bootstrap"),
        WebAuthMode::RefreshToken => Some("refresh-token"),
        WebAuthMode::ApiKey => Some("api-key"),
        WebAuthMode::IngressToken => Some("ingress-token"),
        WebAuthMode::AgentToken => Some("agent-token"),
        WebAuthMode::OAuth => Some("oauth"),
        WebAuthMode::DualToken => Some("dual-token"),
        WebAuthMode::Compatibility => Some("compatibility"),
    }
}

/// Builds the request log record for one web call. `trace_id` resolution mirrors
/// the framework's ResponseIdentity stage (`traceparent` trace id, request id
/// fallback); paths are redacted route templates; query parameters and request
/// headers were redacted/allow-listed in `before`.
pub fn request_log_record_from_state(
    state: &WebCallState,
    status_code: u16,
    service: Option<&str>,
    environment: Option<&str>,
) -> RequestLogRecord {
    let trace_id = state
        .traceparent
        .as_deref()
        .and_then(trace_id_from_traceparent)
        .map(str::to_owned)
        .or_else(|| state.request_id_value().map(str::to_owned));

    RequestLogRecord {
        trace_id: trace_id.unwrap_or_else(|| "unknown".to_owned()),
        request_id: state.request_id_value().unwrap_or("unknown").to_owned(),
        tenant_id: state
            .principal
            .as_ref()
            .map(|principal| principal.tenant_id().to_owned()),
        user_id: state
            .principal
            .as_ref()
            .map(|principal| principal.user_id().to_owned()),
        api_surface: match state.api_surface {
            sdkwork_web_core::WebApiSurface::OpenApi => LogApiSurface::OpenApi,
            sdkwork_web_core::WebApiSurface::AppApi => LogApiSurface::AppApi,
            sdkwork_web_core::WebApiSurface::BackendApi => LogApiSurface::BackendApi,
            sdkwork_web_core::WebApiSurface::InternalApi => LogApiSurface::InternalApi,
            sdkwork_web_core::WebApiSurface::GatewayApi => LogApiSurface::GatewayApi,
            sdkwork_web_core::WebApiSurface::Unknown => LogApiSurface::Unknown,
        },
        path: redact_path_template(&state.path),
        method: state.method.clone(),
        operation_id: state.operation_id.clone(),
        service: service.map(str::to_owned),
        environment: environment.map(str::to_owned),
        auth_mode: web_auth_mode_str(&state.auth_mode).map(str::to_owned),
        status_code: Some(status_code),
        duration_ms: state.accepted_at.map(|started| started.elapsed().as_millis() as u64),
        error_code: state
            .before_failure
            .as_ref()
            .map(|error| error.result_code()),
        failed_stage: state
            .before_failure
            .as_ref()
            .and_then(|error| error.failed_stage.clone()),
        query_params: state.redacted_query.clone(),
        request_headers: state.safe_request_headers.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        extract::Request,
        http::{header, Method},
    };
    use sdkwork_web_core::{
        new_request_id, ServerRequestId, WebApiSurface, WebAuthLevel, WebDeploymentMode,
        WebLoginScope, WebRequestPrincipal, WebSubjectType,
    };

    fn state_with_traceparent() -> WebCallState {
        let mut request = Request::builder()
            .method(Method::GET)
            .uri("/backend/v3/api/log/request_logs?trace_id=x&token=secret&page_size=20")
            .header(header::USER_AGENT, "test-agent")
            .header("x-request-id", "req-1")
            .body(Body::empty())
            .expect("request");
        request
            .headers_mut()
            .insert("x-request-id", "req-1".parse().expect("header"));
        let mut state = WebCallState::from_request(&request);
        state.request_id = Some(ServerRequestId(new_request_id()));
        state.traceparent = Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_owned());
        state.api_surface = WebApiSurface::BackendApi;
        state.operation_id = Some("log.requestLogs.list".to_owned());
        state.accepted_at = Some(std::time::Instant::now());
        state
    }

    #[test]
    fn record_uses_traceparent_trace_id() {
        let state = state_with_traceparent();
        let record = request_log_record_from_state(&state, 200, Some("svc-test"), Some("prod"));
        assert_eq!("4bf92f3577b34da6a3ce929d0e0e4736", record.trace_id);
        assert_eq!("200", record.status_code.unwrap().to_string());
        assert_eq!(LogApiSurface::BackendApi, record.api_surface);
        assert_eq!(
            "/backend/v3/api/log/request_logs",
            record.path,
            "query string must not leak into the stored path"
        );
        assert_eq!(Some("log.requestLogs.list".to_owned()), record.operation_id);
        assert_eq!(Some("svc-test".to_owned()), record.service);
        assert_eq!(Some("prod".to_owned()), record.environment);
        assert_eq!(Some("public".to_owned()), record.auth_mode);
    }

    #[test]
    fn record_falls_back_to_request_id_when_no_traceparent() {
        let mut state = state_with_traceparent();
        state.traceparent = None;
        let record = request_log_record_from_state(&state, 500, None, None);
        assert_eq!(
            state.request_id_value().unwrap(),
            record.trace_id,
            "request id fallback mirrors the framework ResponseIdentity stage"
        );
    }

    #[test]
    fn record_carries_principal_tenant_and_user() {
        let mut state = state_with_traceparent();
        state.principal = Some(
            WebRequestPrincipal::builder()
                .tenant_id("100001")
                .organization_id(Some("0".to_owned()))
                .login_scope(WebLoginScope::Tenant)
                .user_id("user-1")
                .session_id(Some("session-1".to_owned()))
                .app_id("log-console")
                .environment(WebEnvironment::Prod)
                .deployment_mode(WebDeploymentMode::Saas)
                .auth_level(WebAuthLevel::Password)
                .subject_type(WebSubjectType::User)
                .build(),
        );
        let record = request_log_record_from_state(&state, 201, None, None);
        assert_eq!(Some("100001".to_owned()), record.tenant_id);
        assert_eq!(Some("user-1".to_owned()), record.user_id);
    }

    #[test]
    fn record_captures_before_stage_error_code_and_stage() {
        let mut state = state_with_traceparent();
        state.before_failure = Some(
            WebFrameworkError::missing_credentials("no credentials")
                .with_auth_diagnostics(Some("dual-token"), "request_context_resolution"),
        );
        let record = request_log_record_from_state(&state, 401, None, None);
        assert_eq!(Some(40101), record.error_code);
        assert_eq!(
            Some("request_context_resolution".to_owned()),
            record.failed_stage
        );
    }

    #[test]
    fn record_carries_redacted_query_params() {
        let mut state = state_with_traceparent();
        state.redacted_query = redact_query_params(Some("page=2&token=secret"));
        let record = request_log_record_from_state(&state, 200, None, None);
        assert_eq!(Some("page=2&token=[REDACTED]".to_owned()), record.query_params);
        assert!(!record.query_params.as_deref().unwrap().contains("secret"));
    }
}
