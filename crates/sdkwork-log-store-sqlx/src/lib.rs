//! SQLx store adapter for the SDKWork log foundation (`log_request` table).
//!
//! PostgreSQL is the default authoritative-server backend. SQLite support is
//! compiled only when an explicitly declared client-local consumer enables it.
//!
//! Initialization state: authoritative PostgreSQL DDL lives in the module
//! baseline (`database/ddl/baseline/postgres/0001_log_baseline.sql`) and is
//! applied by `sdkwork-log-database-host` through the lifecycle orchestrator;
//! this crate never runs migrations on PostgreSQL.

mod pool;
mod purge;
mod store;

pub use pool::LogStorePool;
pub use store::{SqlxRequestLogStore, DEFAULT_LOG_TTL_SECS};

/// Epoch seconds now (wall-clock); storage timestamps use `INTEGER`/`BIGINT`
/// epoch seconds consistent with the web framework webstore tables.
pub fn now_epoch_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// Open a SQLite pool through `sdkwork-database-sqlx`, run embedded migrations,
/// and return it.
#[cfg(feature = "sqlite")]
pub async fn connect_sqlite(
    database_url: &str,
    max_connections: u32,
) -> Result<sqlx::SqlitePool, sqlx::Error> {
    use sdkwork_database_config::{DatabaseConfig, DatabaseEngine, DeploymentMode};
    use sdkwork_database_sqlx::PoolBuilder;
    let mut config = DatabaseConfig {
        engine: DatabaseEngine::Sqlite,
        url: database_url.to_string(),
        mode: DeploymentMode::Standalone,
        max_connections: max_connections.max(1),
        ..DatabaseConfig::default()
    };
    config.sqlite.create_if_missing = true;

    let db_pool = PoolBuilder::new(config)
        .build()
        .await
        .map_err(|error| sqlx::Error::Configuration(error.to_string().into()))?;

    let pool = db_pool
        .as_sqlite()
        .cloned()
        .ok_or_else(|| sqlx::Error::Configuration("expected sqlite pool".into()))?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
