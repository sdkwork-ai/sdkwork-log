-- Incremental migration for stores whose log_request table predates identity
-- capture: adds the authenticated user display-name snapshot and the masked /
-- hashed client IP columns (SQLite dialect, mirrored by the PostgreSQL GA
-- migration 0002_log_request_user_ip.up.sql).
--
-- Raw client IP addresses are personal data and are never persisted
-- (DATABASE_SPEC §18): client_ip_masked hides the last octet (`1.2.3.x`,
-- IPv6 `/64` subnet) and client_ip_hash is the SHA-256 hex digest used for
-- exact-match lookups.

ALTER TABLE log_request ADD COLUMN user_name TEXT;
ALTER TABLE log_request ADD COLUMN client_ip_hash TEXT;
ALTER TABLE log_request ADD COLUMN client_ip_masked TEXT;
