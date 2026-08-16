-- Add authenticated user display-name snapshot and masked / hashed client IP
-- capture columns to log_request.
--   - user_name: snapshot of the authenticated subject's display name
--     (`iam_user.display_name`) captured at request time.
--   - client_ip_hash: SHA-256 hex digest for exact-match lookups.
--   - client_ip_masked: display-safe masked IP (`1.2.3.x`, IPv6 `/64` subnet).
-- Raw IP addresses are personal data and are never persisted (DATABASE_SPEC
-- §18); the schema.yaml contract version bumps to 1.2.0 with this change.
-- sdkwork:migration
-- id: 0002_log_request_user_ip
-- engine: postgres
-- module: log
-- purpose: Add user_name and masked/hashed client_ip columns to log_request.
-- reversible: true
-- rollback: down-migration
-- transactional: true
-- lock: table
-- lock_timeout: 2s
-- statement_timeout: 60s
-- contract_version: 1.2.0

BEGIN;

ALTER TABLE log_request ADD COLUMN IF NOT EXISTS user_name TEXT;
ALTER TABLE log_request ADD COLUMN IF NOT EXISTS client_ip_hash TEXT;
ALTER TABLE log_request ADD COLUMN IF NOT EXISTS client_ip_masked TEXT;

COMMIT;
