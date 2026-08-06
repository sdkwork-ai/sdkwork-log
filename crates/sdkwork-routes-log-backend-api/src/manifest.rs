//! Route manifest for the log backend-api (`WEB_BACKEND_SPEC.md`).

use crate::paths;
use crate::tenant_scope::{PERM_LOG_PLATFORM_READ, PERM_LOG_TENANT_READ};
use sdkwork_web_contract::{HttpMethod, HttpRoute};

pub const ROUTES: &[HttpRoute] = &[
    HttpRoute::dual_token(
        HttpMethod::Get,
        paths::request_logs::PATH,
        "log",
        "log.requestLogs.list",
    )
    .with_required_permission(PERM_LOG_TENANT_READ)
    .with_alternate_permissions(&[PERM_LOG_PLATFORM_READ]),
    HttpRoute::dual_token(
        HttpMethod::Get,
        paths::request_log_detail::PATH,
        "log",
        "log.requestLogs.detail",
    )
    .with_required_permission(PERM_LOG_TENANT_READ)
    .with_alternate_permissions(&[PERM_LOG_PLATFORM_READ]),
];
