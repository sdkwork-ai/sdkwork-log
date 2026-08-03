//! CLI entrypoint: bootstrap and migrate the log database module from env
//! (`LOG_DATABASE_URL` or `SDKWORK_LOG_*` configuration).

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
