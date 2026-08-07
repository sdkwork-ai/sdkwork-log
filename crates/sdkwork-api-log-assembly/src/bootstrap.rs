//! Log API assembly bootstrap.
//!
//! The assembly owns the request log query router (`/backend/v3/api/log/*`),
//! the per-request capture layer (one `log_request` row per HTTP request with
//! redacted bodies), and the `log_request` module database lifecycle. The
//! lifecycle runs against the host-injected process pool — the consuming
//! application passes its canonical pool handle, never a second pool
//! (`DATABASE_SPEC_PROCESS_SHARED_POOL.md` §2/§4, `API_ASSEMBLY_SPEC.md` §4).
//! Hosts consume the dependency surface through these entrypoints instead of
//! importing `sdkwork-routes-*` directly (`API_ASSEMBLY_SPEC.md` §3/§6.1).

use std::sync::Arc;

use axum::Router;
use http::Extensions;
use sdkwork_database_sqlx::DatabasePool;
use sdkwork_log_core::RequestLogStore;
use sdkwork_log_database_host::{bootstrap_log_database, bootstrap_log_database_from_env};
use sdkwork_log_store_sqlx::SqlxRequestLogStore;
use sdkwork_log_tower_adapter::RequestLoggingLayer;
use sdkwork_web_bootstrap::{ApiAssemblyContribution, ReadinessCheck, ReadinessFuture};
use sdkwork_web_core::{HttpRouteManifest, WebRequestContext};

/// Default request-log service identity recorded on captured rows.
pub const DEFAULT_LOG_SERVICE_NAME: &str = "sdkwork-log";

pub type ApiAssembly = ApiAssemblyContribution;

/// Backend business surface: the query router plus the capture layer the
/// host applies around its complete backend router (one row per request).
pub struct LogBackendAssembly {
    pub router: Router,
    pub capture_layer: RequestLoggingLayer,
}

/// Query-router-only composition for hosts that mount the capture layer
/// separately.
pub struct BusinessRouterAssembly {
    pub router: Router,
}

/// Service host owning the log module lifecycle, pool handle, and store.
#[derive(Clone)]
pub struct LogServiceHost {
    database: sdkwork_log_database_host::LogDatabaseHost,
    store: Arc<dyn RequestLogStore>,
}

impl LogServiceHost {
    /// Bootstraps the `log_request` module lifecycle on the host-injected
    /// process pool (baseline is idempotent; `SDKWORK_DATABASE_AUTO_MIGRATE`
    /// additionally runs pending migrations).
    pub async fn from_pool(pool: &DatabasePool) -> Result<Self, String> {
        let database = bootstrap_log_database(pool.clone())
            .await
            .map_err(|error| format!("log database lifecycle bootstrap failed: {error}"))?;
        let store = postgres_request_log_store(database.pool())?;
        Ok(Self { database, store })
    }

    /// Standalone-process entry: resolves the unified workspace
    /// `SDKWORK_DATABASE_*` profile and the log module root
    /// (`SDKWORK_LOG_APP_ROOT`, falling back to this repository).
    pub async fn from_env() -> Result<Self, String> {
        let database = bootstrap_log_database_from_env()
            .await
            .map_err(|error| format!("log database lifecycle bootstrap failed: {error}"))?;
        let store = postgres_request_log_store(database.pool())?;
        Ok(Self { database, store })
    }

    pub fn database_pool(&self) -> &DatabasePool {
        self.database.pool()
    }

    pub fn store(&self) -> Arc<dyn RequestLogStore> {
        self.store.clone()
    }
}

/// The log module's route inventory for host manifest composition.
pub fn log_route_manifest() -> HttpRouteManifest {
    HttpRouteManifest::new(sdkwork_routes_log_backend_api::ROUTES)
}

pub async fn assemble_api_router(host: Arc<LogServiceHost>) -> Result<ApiAssembly, String> {
    let routes = sdkwork_routes_log_backend_api::ROUTES.to_vec();
    let contribution = ApiAssembly::from_manifest(
        "sdkwork-log",
        "SDKWork Log Request Log API",
        assemble_backend_business_router(host.clone(), DEFAULT_LOG_SERVICE_NAME).router,
        HttpRouteManifest::from_owned_routes(routes),
        Vec::new(),
        Arc::new(LogReadiness {
            pool: host.database_pool().clone(),
        }),
    )?;
    Ok(contribution)
}

pub async fn assemble_api_router_from_env() -> Result<ApiAssembly, String> {
    let host = Arc::new(LogServiceHost::from_env().await?);
    assemble_api_router(host).await
}

pub async fn assemble_api_router_with_pool(pool: DatabasePool) -> Result<ApiAssembly, String> {
    let host = Arc::new(LogServiceHost::from_pool(&pool).await?);
    assemble_api_router(host).await
}

/// Compose the log backend business surface (query router + capture layer)
/// on the shared pool owned by the consuming host (same-origin dependency
/// composition, `API_ASSEMBLY_SPEC.md` §6.1).
pub async fn assemble_backend_business_router_with_pool(
    pool: &DatabasePool,
    service: &str,
) -> Result<LogBackendAssembly, String> {
    let host = Arc::new(LogServiceHost::from_pool(pool).await?);
    Ok(assemble_backend_business_router(host, service))
}

pub async fn assemble_backend_business_router_from_env() -> Result<LogBackendAssembly, String> {
    let host = Arc::new(LogServiceHost::from_env().await?);
    Ok(assemble_backend_business_router(
        host,
        DEFAULT_LOG_SERVICE_NAME,
    ))
}

pub fn assemble_backend_business_router(
    host: Arc<LogServiceHost>,
    service: &str,
) -> LogBackendAssembly {
    let store = host.store();
    LogBackendAssembly {
        router: sdkwork_routes_log_backend_api::build_router(store.clone()),
        capture_layer: RequestLoggingLayer::new(store)
            .with_service(service)
            .with_tenant_resolver(principal_tenant_user_resolver),
    }
}

/// Resolves `(tenant_id, user_id)` from the web-framework principal injected
/// by the outer runtime layer, so tenant isolation matches the authenticated
/// admin subject boundary.
fn principal_tenant_user_resolver(
    extensions: &Extensions,
) -> (Option<String>, Option<String>) {
    let principal = extensions
        .get::<WebRequestContext>()
        .and_then(|context| context.principal());
    (
        principal.map(|value| value.tenant_id().to_owned()),
        principal.map(|value| value.user_id().to_owned()),
    )
}

fn postgres_request_log_store(
    pool: &DatabasePool,
) -> Result<Arc<dyn RequestLogStore>, String> {
    let postgres = pool.as_postgres().cloned().ok_or_else(|| {
        "sdkwork-log assembly requires a PostgreSQL process pool".to_owned()
    })?;
    Ok(Arc::new(SqlxRequestLogStore::new_postgres(postgres)))
}

/// Readiness contribution without public probe-path ownership: verifies the
/// log module pool answers `SELECT 1`.
#[derive(Clone)]
pub struct LogReadiness {
    pool: DatabasePool,
}

impl ReadinessCheck for LogReadiness {
    fn check(&self) -> ReadinessFuture<'_> {
        let pool = self.pool.clone();
        Box::pin(async move {
            match pool.test_connection().await {
                Ok(true) => Ok(()),
                Ok(false) => Err("log database readiness query returned no row".to_owned()),
                Err(error) => Err(format!("log database readiness check failed: {error}")),
            }
        })
    }
}
