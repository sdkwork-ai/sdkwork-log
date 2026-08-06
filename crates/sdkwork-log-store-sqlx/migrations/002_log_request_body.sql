-- Incremental migration for stores whose log_request table predates body
-- capture: adds the redacted request/response body columns (SQLite dialect,
-- mirrored by the PostgreSQL GA migration 0001_log_request_body.up.sql).
--
-- Only redacted body text is ever stored (DATABASE_SPEC §18): raw tokens,
-- passwords, secrets, and full sensitive payloads MUST NOT be persisted.

ALTER TABLE log_request ADD COLUMN request_body TEXT;
ALTER TABLE log_request ADD COLUMN response_body TEXT;
