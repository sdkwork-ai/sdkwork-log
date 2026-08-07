//! SDKWork Log database lifecycle bootstrap (`log_request` module) via
//! `sdkwork-database` (`DATABASE_SPEC.md` §32–§34: production pools MUST go
//! through `sdkwork-database-sqlx` and the lifecycle orchestrator).

use sdkwork_database_config::DatabaseConfig;
use sdkwork_database_lifecycle::{lifecycle_options_from_env, LifecycleOrchestrator};
use sdkwork_database_spi::{DatabaseAssetProvider, DatabaseManifest, DefaultDatabaseModule};
use sdkwork_database_sqlx::{create_pool_from_config, DatabasePool};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct LogDatabaseHost {
    pool: DatabasePool,
    module: Arc<DefaultDatabaseModule>,
}

impl LogDatabaseHost {
    pub fn pool(&self) -> &DatabasePool {
        &self.pool
    }

    pub fn module(&self) -> Arc<DefaultDatabaseModule> {
        self.module.clone()
    }
}

pub async fn bootstrap_log_database(pool: DatabasePool) -> Result<LogDatabaseHost, String> {
    let app_root = resolve_app_root();
    let module = Arc::new(
        DefaultDatabaseModule::from_app_root(&app_root)
            .map_err(|error| format!("load log database module failed: {error}"))?,
    );
    let manifest = DatabaseManifest::from_file(module.manifest_path())
        .map_err(|error| format!("read log database manifest failed: {error}"))?;
    let options = lifecycle_options_from_env("LOG", &manifest);
    let orchestrator = LifecycleOrchestrator::new(pool.clone(), module.clone())
        .with_applied_by("sdkwork-log");

    orchestrator
        .init()
        .await
        .map_err(|error| format!("log database init failed: {error}"))?;

    if options.auto_migrate {
        orchestrator
            .migrate()
            .await
            .map_err(|error| format!("log database migrate failed: {error}"))?;
    }

    Ok(LogDatabaseHost { pool, module })
}

pub async fn bootstrap_log_database_from_env() -> Result<LogDatabaseHost, String> {
    let _ = dotenvy::dotenv();
    let config = DatabaseConfig::from_env("LOG")
        .map_err(|error| format!("read log database config failed: {error}"))?;
    let pool = create_pool_from_config(config)
        .await
        .map_err(|error| format!("create log database pool failed: {error}"))?;
    bootstrap_log_database(pool).await
}

fn resolve_app_root() -> PathBuf {
    std::env::var("SDKWORK_LOG_APP_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
        })
}
