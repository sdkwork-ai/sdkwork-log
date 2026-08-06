# LOG database migrations

PostgreSQL GA migrations land here (`NNNN_*.up.sql` / `NNNN_*.down.sql`) after the
baseline is frozen. The current schema is fully described by
`../ddl/baseline/postgres/0001_log_baseline.sql`; embedded per-dialect migrations
live in `crates/sdkwork-log-store-sqlx/migrations/`.

Applied history:

- `0001_log_request_body.up.sql` — add redacted `request_body` / `response_body`
  capture columns (contract 1.1.0). Baseline DDL updated in lockstep.

