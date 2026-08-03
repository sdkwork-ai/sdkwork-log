//! Request log query backend-api (list/search with traceId filters).
//!
//! Mount the router with a [`RequestLogStore`] (for example
//! [`SqlxRequestLogStore`]): every request to `GET /backend/v3/api/log/request_logs`
//! is served through the standard `SdkWorkApiResponse` envelope and offset
//! pagination (`API_SPEC.md` §14–§16, `PAGINATION_SPEC.md`).

pub mod dto;
pub mod handlers;
pub mod manifest;
pub mod pagination;
pub mod paths;
pub mod response;
pub mod services;
pub mod state;
pub mod tenant_scope;

pub use manifest::ROUTES;
pub use state::LogQueryState;

use axum::routing::get;
use std::sync::Arc;

/// Builds the axum router for the request log query API over the given store.
///
/// The router is intentionally free of framework middleware — the hosting
/// application wraps it with `sdkwork-web-framework` (`with_web_request_context`)
/// exactly once (`WEB_FRAMEWORK_SPEC.md`).
pub fn build_router(store: Arc<dyn sdkwork_log_core::RequestLogStore>) -> axum::Router {
    axum::Router::new()
        .route(
            paths::request_logs::PATH,
            get(handlers::list_request_logs),
        )
        .with_state(state::LogQueryState::from_store(store))
}
