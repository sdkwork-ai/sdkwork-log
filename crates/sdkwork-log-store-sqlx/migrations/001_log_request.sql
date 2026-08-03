CREATE TABLE IF NOT EXISTS log_request (
    id TEXT PRIMARY KEY NOT NULL,
    trace_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    tenant_id TEXT,
    user_id TEXT,
    api_surface TEXT NOT NULL,
    path TEXT NOT NULL,
    method TEXT NOT NULL,
    operation_id TEXT,
    service TEXT,
    environment TEXT,
    auth_mode TEXT,
    status_code INTEGER,
    duration_ms INTEGER,
    error_code INTEGER,
    failed_stage TEXT,
    query_params TEXT,
    request_headers TEXT,
    created_at INTEGER NOT NULL,
    expires_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_log_request_created
    ON log_request (created_at);

CREATE INDEX IF NOT EXISTS idx_log_request_trace
    ON log_request (trace_id);

CREATE INDEX IF NOT EXISTS idx_log_request_request
    ON log_request (request_id);

CREATE INDEX IF NOT EXISTS idx_log_request_tenant
    ON log_request (tenant_id);

CREATE INDEX IF NOT EXISTS idx_log_request_expires
    ON log_request (expires_at);
