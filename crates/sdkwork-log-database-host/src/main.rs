//! CLI entrypoint: bootstrap and migrate the log database module from the
//! unified workspace `SDKWORK_DATABASE_*` PostgreSQL profile
//! (ENVIRONMENT_SPEC section 7.1; the workspace database configuration
//! directory per section 7.3). Retired per-application database keys are
//! rejected by `sdkwork-database-config`.

use sdkwork_database_spi::DatabaseAssetProvider;
use sdkwork_log_database_host::bootstrap_log_database_from_env;

#[tokio::main]
async fn main() -> Result<(), String> {
    let host = bootstrap_log_database_from_env().await?;
    println!(
        "sdkwork-log database ready (module {})",
        host.module().manifest_path().display()
    );
    Ok(())
}
