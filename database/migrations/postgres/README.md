# LOG database migrations

PostgreSQL GA migrations land here (`NNNN_*.up.sql` / `NNNN_*.down.sql`) after the
baseline is frozen. The current schema is fully described by
`../ddl/baseline/postgres/0001_log_baseline.sql`; embedded per-dialect migrations
live in `crates/sdkwork-log-store-sqlx/migrations/`.
