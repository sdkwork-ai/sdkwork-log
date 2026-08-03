//! Route manifest contract for the log backend-api
//! (`sdkwork-web-contract` ↔ `apis/backend-api/log/routes.manifest.json`).

use sdkwork_routes_log_backend_api::paths;
use sdkwork_routes_log_backend_api::ROUTES;
use sdkwork_web_contract::HttpMethod;
use serde_json::Value;

#[test]
fn manifest_contract_matches_committed_routes_manifest() {
    let committed = read_json(authority_dir().join("routes.manifest.json"));
    let committed: Vec<Value> = committed
        .as_array()
        .cloned()
        .expect("routes.manifest.json must be a JSON array");
    assert_eq!(
        manifest_rows(),
        committed,
        "apis/backend-api/log/routes.manifest.json is stale; run \
         cargo test -p sdkwork-routes-log-backend-api materialize_openapi_authority_file -- --ignored"
    );
}

#[test]
fn manifest_declares_request_log_list_route() {
    assert_eq!(1, ROUTES.len());
    let route = &ROUTES[0];
    assert_eq!(HttpMethod::Get, route.method);
    assert_eq!(paths::request_logs::PATH, route.path);
    assert_eq!("log.requestLogs.list", route.operation_id);
}

fn authority_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apis/backend-api/log")
}

fn read_json(path: std::path::PathBuf) -> Value {
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn manifest_rows() -> Vec<Value> {
    ROUTES
        .iter()
        .map(|route| {
            let mut row = serde_json::json!({
                "method": method_label(route.method),
                "path": route.path,
                "operationId": route.operation_id,
                "auth": auth_mode_label(route.auth),
                "apiSurface": "backend-api",
                "requestContext": "WebRequestContext",
                "forbidCredentialHeaders": route.forbid_credential_headers,
                "requiredPermission": route.required_permission,
            });
            if let Some(alternate) = route.alternate_permissions {
                row["alternatePermissions"] = serde_json::json!(alternate);
            }
            row
        })
        .collect()
}

fn method_label(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Delete => "delete",
        HttpMethod::Get => "get",
        HttpMethod::Patch => "patch",
        HttpMethod::Post => "post",
        HttpMethod::Put => "put",
    }
}

fn auth_mode_label(auth: sdkwork_web_contract::RouteAuth) -> &'static str {
    use sdkwork_web_contract::RouteAuth;
    match auth {
        RouteAuth::Public => "public",
        RouteAuth::BootstrapBody => "bootstrap-body",
        RouteAuth::CredentialEntryBootstrap => "credential-entry-bootstrap",
        RouteAuth::DualToken => "dual-token",
        RouteAuth::ApiKey => "api-key",
        RouteAuth::IngressToken => "ingress-token",
        RouteAuth::OAuth => "oauth",
        RouteAuth::OpenApiFlexible => "open-api-flexible",
        RouteAuth::ApiKeyOrDualToken => "api-key-or-dual-token",
        RouteAuth::RefreshToken => "refresh-token",
        RouteAuth::AgentToken => "agent-token",
        RouteAuth::Compatibility => "compatibility",
    }
}
