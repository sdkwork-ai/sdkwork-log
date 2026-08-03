use axum::{
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use sdkwork_utils_rust::SdkWorkApiResponse;
use sdkwork_web_core::{
    problem_response, WebFrameworkError, WebFrameworkErrorKind, WebRequestContext,
};
use serde::Serialize;

pub type ApiResult<T> = Result<T, ApiProblem>;

pub fn ok_json<T>(data: T) -> ApiResult<T> {
    Ok(data)
}

pub fn success_json<T: Serialize>(
    ctx: &WebRequestContext,
    data: T,
) -> Result<Response, ApiProblem> {
    success_response(ctx, StatusCode::OK, data)
}

fn success_response<T: Serialize>(
    ctx: &WebRequestContext,
    status: StatusCode,
    data: T,
) -> Result<Response, ApiProblem> {
    let trace_id = ctx.resolved_trace_id();
    let envelope = SdkWorkApiResponse::success(data, trace_id.clone());
    let mut response = (status, Json(envelope)).into_response();
    attach_trace_header(&mut response, &trace_id);
    Ok(response)
}

fn attach_trace_header(response: &mut Response, trace_id: &str) {
    if let Ok(value) = HeaderValue::from_str(trace_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-sdkwork-trace-id"), value);
    }
}

#[derive(Debug)]
pub struct ApiProblem {
    pub message: String,
    status: StatusCode,
}

impl ApiProblem {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: StatusCode::BAD_REQUEST,
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: StatusCode::FORBIDDEN,
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: StatusCode::NOT_FOUND,
        }
    }

    pub fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    pub fn from_web_framework(error: sdkwork_web_core::WebFrameworkError) -> Self {
        let status = error.status();
        Self {
            message: error.message,
            status,
        }
    }

    fn framework_error(&self) -> WebFrameworkError {
        let kind = match self.status {
            StatusCode::BAD_REQUEST => WebFrameworkErrorKind::BadRequest,
            StatusCode::UNPROCESSABLE_ENTITY => WebFrameworkErrorKind::UnprocessableEntity,
            StatusCode::FORBIDDEN => WebFrameworkErrorKind::Forbidden,
            StatusCode::NOT_FOUND => WebFrameworkErrorKind::NotFound,
            StatusCode::CONFLICT => WebFrameworkErrorKind::Conflict,
            StatusCode::PAYLOAD_TOO_LARGE => WebFrameworkErrorKind::PayloadTooLarge,
            StatusCode::TOO_MANY_REQUESTS => WebFrameworkErrorKind::RateLimitExceeded,
            StatusCode::SERVICE_UNAVAILABLE => WebFrameworkErrorKind::DependencyUnavailable,
            StatusCode::REQUEST_TIMEOUT => WebFrameworkErrorKind::RequestTimeout,
            StatusCode::METHOD_NOT_ALLOWED => WebFrameworkErrorKind::MethodNotAllowed,
            StatusCode::UNAUTHORIZED => WebFrameworkErrorKind::MissingCredentials,
            StatusCode::INTERNAL_SERVER_ERROR => WebFrameworkErrorKind::InternalServerError,
            _ => WebFrameworkErrorKind::InternalServerError,
        };
        WebFrameworkError {
            kind,
            message: self.message.clone(),
            retry_after_seconds: None,
            auth_profile: None,
            failed_stage: None,
            reason: None,
        }
    }

    pub fn into_response_for(&self, ctx: &WebRequestContext) -> Response {
        problem_response(&self.framework_error(), ctx.problem_correlation())
    }
}

/// Finish a JSON handler `Result` with request-scoped Problem correlation.
pub fn finish_api_json<T: Serialize>(ctx: &WebRequestContext, result: ApiResult<T>) -> Response {
    match result {
        Ok(data) => {
            success_json(ctx, data).unwrap_or_else(|problem| problem.into_response_for(ctx))
        }
        Err(problem) => problem.into_response_for(ctx),
    }
}
