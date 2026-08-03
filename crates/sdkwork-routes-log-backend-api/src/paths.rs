//! Route paths for the log backend-api.

pub mod request_logs {
    /// Request log list/search (offset pagination, traceId filters).
    pub const PATH: &str = "/backend/v3/api/log/request_logs";
}
