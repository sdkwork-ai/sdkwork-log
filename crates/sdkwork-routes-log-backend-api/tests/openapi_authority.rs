//! OpenAPI authority contract for the log backend-api.

use sdkwork_routes_log_backend_api::paths;
use sdkwork_routes_log_backend_api::ROUTES;
use sdkwork_web_contract::{
    build_openapi_document, validate_openapi_document_context_selectors,
    validate_openapi_routes_context_selectors, HttpMethod, OPENAPI_API_SURFACE_EXTENSION,
    OPENAPI_AUTH_MODE_EXTENSION, OPENAPI_PERMISSION_EXTENSION, OPENAPI_REQUEST_CONTEXT_EXTENSION,
};
use serde_json::{json, Value};

/// SDK family metadata injected into the committed OpenAPI authority so it can
/// serve directly as an SDK generation input (`x-api-prefix`, `x-sdk-family`,
/// `x-sdk-client`, servers, security, tags).
fn authority_openapi_document() -> Value {
    let mut doc = build_openapi_document("SDKWork Log Backend API", ROUTES);
    if let Value::Object(map) = &mut doc {
        map.insert(
            "jsonSchemaDialect".to_owned(),
            json!("https://json-schema.org/draft/2020-12/schema"),
        );
        map.insert("x-api-prefix".to_owned(), json!("/backend/v3/api"));
        map.insert("x-sdk-family".to_owned(), json!("sdkwork-log-backend-sdk"));
        map.insert("x-sdk-client".to_owned(), json!("SdkworkBackendClient"));
        map.insert(
            "servers".to_owned(),
            json!([{ "description": "Local backend API server", "url": "http://localhost:18081" }]),
        );
        map.insert(
            "security".to_owned(),
            json!([{ "AccessToken": [], "AuthToken": [] }]),
        );
        let mut tag_names = std::collections::BTreeSet::new();
        for route in ROUTES {
            tag_names.insert(route.tag);
        }
        map.insert(
            "tags".to_owned(),
            json!(tag_names
                .iter()
                .map(|tag| json!({ "name": tag, "description": format!("{tag} operations exposed by the log foundation.") }))
                .collect::<Vec<_>>()),
        );
    }
    // Inject the log-specific list filters (the framework generator emits only
    // the generic list parameters). Schemas follow `API_SPEC.md` §13:
    // int64-as-string for epoch-second range filters.
    if let Some(paths) = doc["paths"].as_object_mut() {
        if let Some(operation) = paths[paths::request_logs::PATH]
            .get_mut("get")
            .and_then(Value::as_object_mut)
        {
            let parameters = operation.entry("parameters").or_insert_with(|| json!([]));
            if let Some(parameters) = parameters.as_array_mut() {
                let filters = [
                    ("trace_id", json!({ "type": "string" })),
                    ("request_id", json!({ "type": "string" })),
                    (
                        "api_surface",
                        json!({
                            "type": "string",
                            "enum": ["open-api", "app-api", "backend-api", "internal-api", "gateway-api", "unknown"]
                        }),
                    ),
                    ("operation_id", json!({ "type": "string" })),
                    ("service", json!({ "type": "string" })),
                    (
                        "status",
                        json!({ "type": "integer", "minimum": 100, "maximum": 599 }),
                    ),
                    ("created_from", json!({ "type": "string", "pattern": "^[0-9]+$" })),
                    ("created_to", json!({ "type": "string", "pattern": "^[0-9]+$" })),
                ];
                for (name, schema) in filters {
                    parameters.push(json!({
                        "in": "query",
                        "name": name,
                        "required": false,
                        "schema": schema,
                    }));
                }
            }
        }
    }
    doc
}

#[test]
fn openapi_authority_matches_manifest_contract() {
    let doc = authority_openapi_document();
    let paths = doc["paths"].as_object().expect("paths object");
    assert_eq!(ROUTES.len(), count_operations(paths));

    let sample = paths[paths::request_logs::PATH]
        .as_object()
        .expect("request logs path")["get"]
        .as_object()
        .expect("get operation");
    assert_eq!(
        "WebRequestContext",
        sample[OPENAPI_REQUEST_CONTEXT_EXTENSION].as_str().unwrap()
    );
    assert_eq!(
        "backend-api",
        sample[OPENAPI_API_SURFACE_EXTENSION].as_str().unwrap()
    );
    assert_eq!(
        "dual-token",
        sample[OPENAPI_AUTH_MODE_EXTENSION].as_str().unwrap()
    );
    validate_openapi_routes_context_selectors(ROUTES).expect("manifest paths");
    validate_openapi_document_context_selectors(&doc).expect("materialized openapi");
    validate_openapi_document_context_selectors(&read_json(authority_dir().join("openapi.json")))
        .expect("committed openapi authority");
}

#[test]
fn committed_openapi_authority_matches_runtime_contract() {
    let expected = authority_openapi_document();
    let authority_dir = authority_dir();
    let committed = read_json(authority_dir.join("openapi.json"));
    assert_eq!(
        expected, committed,
        "apis/backend-api/log/openapi.json is stale; run \
         cargo test -p sdkwork-routes-log-backend-api materialize_openapi_authority_file -- --ignored"
    );
}

#[test]
fn committed_openapi_declares_success_on_get_routes() {
    assert_openapi_responses_on_all_routes("200", "Success");
}

#[test]
fn committed_openapi_declares_unauthorized_on_all_routes() {
    assert_openapi_responses_on_all_routes("401", "Unauthorized");
}

#[test]
fn committed_openapi_declares_forbidden_on_all_routes() {
    assert_openapi_responses_on_all_routes("403", "Forbidden");
}

#[test]
fn committed_openapi_declares_rate_limit_on_all_routes() {
    assert_openapi_responses_on_all_routes("429", "Too Many Requests");
}

#[test]
fn committed_openapi_declares_internal_error_on_all_routes() {
    assert_openapi_responses_on_all_routes("500", "Internal Server Error");
}

#[test]
fn committed_openapi_declares_dependency_unavailable_on_all_routes() {
    assert_openapi_responses_on_all_routes("503", "Service Unavailable");
}

#[test]
fn committed_openapi_declares_permission_extensions() {
    let committed = read_json(authority_dir().join("openapi.json"));
    let paths = committed["paths"]
        .as_object()
        .expect("openapi paths must be an object");
    for route in ROUTES {
        let Some(permission) = route.required_permission else {
            continue;
        };
        let path_entry = paths
            .get(route.path)
            .unwrap_or_else(|| panic!("openapi missing path {}", route.path));
        let method = method_label(route.method);
        let operation = path_entry
            .get(method)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("openapi missing {method} on {}", route.path));
        assert_eq!(
            permission,
            operation
                .get(OPENAPI_PERMISSION_EXTENSION)
                .and_then(Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "{method} {} must declare {OPENAPI_PERMISSION_EXTENSION}",
                        route.path
                    )
                })
        );
    }
}

#[test]
#[ignore = "run manually to refresh apis/backend-api/log/openapi.json"]
fn materialize_openapi_authority_file() {
    let doc = authority_openapi_document();
    let rendered = serde_json::to_string_pretty(&doc).expect("serialize openapi");
    let manifest = serde_json::to_string_pretty(&manifest_rows()).expect("serialize manifest");
    let root = authority_dir();
    std::fs::create_dir_all(&root).expect("create authority dir");
    std::fs::write(root.join("openapi.json"), rendered).expect("write openapi.json");
    std::fs::write(root.join("routes.manifest.json"), manifest).expect("write manifest");
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

fn count_operations(paths: &serde_json::Map<String, Value>) -> usize {
    paths
        .values()
        .filter_map(Value::as_object)
        .map(|methods| methods.len())
        .sum()
}

fn assert_openapi_responses_on_all_routes(status: &str, description: &str) {
    let committed = read_json(authority_dir().join("openapi.json"));
    let paths = committed["paths"]
        .as_object()
        .expect("openapi paths must be an object");
    for route in ROUTES {
        let path_entry = paths
            .get(route.path)
            .unwrap_or_else(|| panic!("openapi missing path {}", route.path));
        let method = method_label(route.method);
        let operation = path_entry
            .get(method)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("openapi missing {method} on {}", route.path));
        let responses = operation
            .get("responses")
            .and_then(Value::as_object)
            .expect("responses");
        assert!(
            responses.contains_key(status),
            "{method} {} must declare {status} {description}",
            route.path
        );
    }
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
        RouteAuth::OpenApiBearerFlexible => "open-api-bearer-flexible",
        RouteAuth::ApiKeyOrDualToken => "api-key-or-dual-token",
        RouteAuth::RefreshToken => "refresh-token",
        RouteAuth::AgentToken => "agent-token",
        RouteAuth::Compatibility => "compatibility",
    }
}
