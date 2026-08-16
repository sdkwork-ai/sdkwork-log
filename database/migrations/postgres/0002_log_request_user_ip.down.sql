-- Reverse of 0002_log_request_user_ip: drops the user display-name snapshot
-- and masked / hashed client IP columns.
-- sdkwork:migration
-- id: 0002_log_request_user_ip
-- engine: postgres
-- module: log
-- purpose: Drop user_name and masked/hashed client_ip columns from log_request.
-- reversible: true
-- rollback: down-migration
-- transactional: true
-- lock: table
-- lock_timeout: 2s
-- statement_timeout: 60s
-- contract_version: 1.2.0

BEGIN;

ALTER TABLE log_request DROP COLUMN IF EXISTS user_name;
ALTER TABLE log_request DROP COLUMN IF EXISTS client_ip_hash;
ALTER TABLE log_request DROP COLUMN IF EXISTS client_ip_masked;

COMMIT;
