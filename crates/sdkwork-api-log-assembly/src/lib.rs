//! Host-neutral API assembly for sdkwork-log: request log query router,
//! per-request capture layer, and `log_request` module lifecycle bootstrap.

// SDKWORK-ASSEMBLY-LIB-CUSTOM

mod bootstrap;
mod generated;

pub use bootstrap::{
    assemble_api_router, assemble_api_router_from_env, assemble_api_router_with_pool,
    assemble_backend_business_router, assemble_backend_business_router_from_env,
    assemble_backend_business_router_with_pool, log_route_manifest, ApiAssembly,
    BusinessRouterAssembly, LogBackendAssembly, LogServiceHost,
};

pub fn assembly_route_count() -> usize {
    generated::ROUTE_CRATE_COUNT
}
