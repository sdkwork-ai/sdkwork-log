-- SDKWork log foundation baseline (PostgreSQL).
-- Single table `log_request`: one row per recorded HTTP request (all API surfaces,
-- including webhook ingresses). traceId is server-owned and REQUIRED
-- (OBSERVABILITY_SPEC §2: access logs must use traceId). Fields follow the
-- OBSERVABILITY_SPEC §2 SHOULD list (service, environment, auth mode, stage,
-- status, duration); query_params and request_headers are captured redacted /
-- allow-listed only (DATABASE_SPEC §18: sensitive values MUST NOT be stored).
--
-- source: crates/sdkwork-log-store-sqlx/migrations/001_log_request.sql
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
    created_at BIGINT NOT NULL,
    expires_at BIGINT
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
