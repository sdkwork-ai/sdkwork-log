//! Route paths for the log backend-api.

pub mod request_logs {
    /// Request log list/search (offset pagination, traceId filters).
    pub const PATH: &str = "/backend/v3/api/log/request_logs";
}

pub mod request_log_detail {
    /// Request log detail by row id (full redacted request/response bodies).
    pub const PATH: &str = "/backend/v3/api/log/request_logs/{id}";
}
