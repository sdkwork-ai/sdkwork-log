//! Query service over the framework-agnostic [`RequestLogStore`].

use crate::response::ApiProblem;
use sdkwork_log_core::{
    RequestLogListQuery, RequestLogPage, RequestLogRow, RequestLogStore,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct LogQueryService {
    store: Arc<dyn RequestLogStore>,
}

impl LogQueryService {
    pub fn new(store: Arc<dyn RequestLogStore>) -> Self {
        Self { store }
    }

    pub async fn list_request_logs(
        &self,
        query: RequestLogListQuery,
    ) -> Result<RequestLogPage, ApiProblem> {
        self.store.list(query).await.map_err(|error| {
            ApiProblem::dependency_unavailable(format!(
                "request log store error: {}",
                error.message
            ))
        })
    }

    /// Fetches one row by id with the full redacted bodies.
    pub async fn get_request_log(
        &self,
        id: &str,
    ) -> Result<Option<RequestLogRow>, ApiProblem> {
        self.store.get_by_id(id).await.map_err(|error| {
            ApiProblem::dependency_unavailable(format!(
                "request log store error: {}",
                error.message
            ))
        })
    }
}
