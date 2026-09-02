//! Host-neutral API assembly for sdkwork-log: request log query router,
//! per-request capture layer, and `log_request` module lifecycle bootstrap.

// SDKWORK-ASSEMBLY-LIB-CUSTOM

mod bootstrap;
mod generated;

pub use bootstrap::{assemble_api_router, ApiAssembly, assemble_api_router_from_env, assemble_api_router_with_pool, assemble_backend_business_router, assemble_backend_business_router_from_env, assemble_backend_business_router_with_pool, BusinessRouterAssembly, log_route_manifest, LogBackendAssembly, LogServiceHost, web_module, web_module_with_pool};
// Retention policy value types and the surface resolver signature are part of
// the assembly's public contract: hosts build policies and resolver closures
// without importing the log foundation crates directly.
pub use sdkwork_log_core::{
    DEFAULT_LOG_RETENTION_DAYS, LogApiSurface, LogRetention, LogRetentionPolicy, LogRetentionRule,
};
pub use sdkwork_log_tower_adapter::ApiSurfaceResolver;

pub fn assembly_route_count() -> usize {
    generated::ROUTE_CRATE_COUNT
}
