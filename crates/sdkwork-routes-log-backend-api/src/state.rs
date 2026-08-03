use crate::services::LogQueryService;
use sdkwork_log_core::RequestLogStore;
use std::sync::Arc;

#[derive(Clone)]
pub struct LogQueryState {
    pub service: LogQueryService,
}

impl LogQueryState {
    pub fn from_store(store: Arc<dyn RequestLogStore>) -> Self {
        Self {
            service: LogQueryService::new(store),
        }
    }
}
