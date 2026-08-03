//! Tenant scope enforcement for log query handlers (`WEB_BACKEND_SPEC.md`).

use crate::response::ApiProblem;
use sdkwork_web_core::WebRequestContext;

pub const PERM_LOG_TENANT_READ: &str = "log.tenant.read";
pub const PERM_LOG_PLATFORM_READ: &str = "log.platform.read";

/// Requires an authenticated tenant/app principal with `log.tenant.read` or
/// `log.platform.read`.
pub fn require_tenant_read(ctx: &WebRequestContext) -> Result<(), ApiProblem> {
    ctx.require_tenant_id()
        .map_err(ApiProblem::from_web_framework)?;
    ctx.require_app_id()
        .map_err(ApiProblem::from_web_framework)?;
    if ctx.has_permission(PERM_LOG_TENANT_READ) || ctx.has_permission(PERM_LOG_PLATFORM_READ) {
        return Ok(());
    }
    Err(ApiProblem::forbidden(format!(
        "missing required permission: {PERM_LOG_TENANT_READ}"
    )))
}

/// Resolves the tenant scope for list queries — tenant admins never receive
/// cross-tenant or NULL tenant rows.
///
/// - `log.platform.read`: requested `tenant_id` honored; `None` = platform-wide.
/// - otherwise: always the authenticated tenant; a mismatched `tenant_id` query
///   parameter is rejected.
pub fn resolve_list_tenant_id(
    ctx: &WebRequestContext,
    query_tenant_id: Option<&str>,
) -> Result<Option<String>, ApiProblem> {
    if ctx.has_permission(PERM_LOG_PLATFORM_READ) {
        return Ok(query_tenant_id
            .filter(|value| !value.is_empty())
            .map(str::to_owned));
    }
    let tenant = ctx
        .require_tenant_id()
        .map_err(ApiProblem::from_web_framework)?;
    if let Some(requested) = query_tenant_id.filter(|value| !value.is_empty()) {
        if requested != tenant {
            return Err(ApiProblem::forbidden(
                "tenant_id query parameter does not match authenticated tenant",
            ));
        }
    }
    Ok(Some(tenant.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths;
    use sdkwork_web_core::{
        ServerRequestId, WebApiSurface, WebAuthLevel, WebAuthMode, WebDeploymentMode,
        WebEnvironment, WebLoginScope, WebRequestPrincipal, WebSubjectType, WebTransportFacts,
    };

    fn ctx_with_permissions(permissions: &[&str]) -> WebRequestContext {
        let principal = WebRequestPrincipal::builder()
            .tenant_id("100001")
            .organization_id(Some("0".to_owned()))
            .login_scope(WebLoginScope::Tenant)
            .user_id("user-test")
            .session_id(Some("session-test".to_owned()))
            .app_id("log-console")
            .environment(WebEnvironment::Prod)
            .deployment_mode(WebDeploymentMode::Saas)
            .auth_level(WebAuthLevel::Password)
            .subject_type(WebSubjectType::User)
            .permission_scope(permissions.iter().map(|value| (*value).to_owned()).collect())
            .build();
        WebRequestContext {
            request_id: ServerRequestId("req-test".to_owned()),
            api_surface: WebApiSurface::BackendApi,
            auth_mode: WebAuthMode::DualToken,
            principal: Some(principal),
            transport: WebTransportFacts {
                path: paths::request_logs::PATH.to_owned(),
                method: "GET".to_owned(),
                auth_token_present: true,
                access_token_present: true,
                api_key_present: false,
                ingress_token_present: false,
                oauth_bearer_present: false,
                agent_token_present: false,
            },
            locale: None,
            client_kind: None,
            operation: None,
            trace_id: None,
            idempotency_key: None,
        }
    }

    #[test]
    fn tenant_read_scope_allows_own_tenant_only() {
        let ctx = ctx_with_permissions(&["log.tenant.read"]);
        assert_eq!(
            Some("100001".to_owned()),
            resolve_list_tenant_id(&ctx, None).expect("scope")
        );
        assert!(resolve_list_tenant_id(&ctx, Some("100002")).is_err());
    }

    #[test]
    fn platform_read_scope_allows_cross_tenant_and_platform_wide() {
        let ctx = ctx_with_permissions(&["log.tenant.read", "log.platform.read"]);
        assert_eq!(
            Some("100002".to_owned()),
            resolve_list_tenant_id(&ctx, Some("100002")).expect("cross tenant")
        );
        assert_eq!(None, resolve_list_tenant_id(&ctx, None).expect("platform wide"));
    }

    #[test]
    fn require_tenant_read_rejects_missing_permission() {
        let ctx = ctx_with_permissions(&[]);
        assert!(require_tenant_read(&ctx).is_err());
    }
}
