//! Thin HTTP adapters for the log backend-api (`WEB_BACKEND_SPEC.md` §2).

use crate::dto::{AdminLogListQuery, LogRequestDetailEnvelope, LogRequestDetailItem, LogRequestListItem};
use crate::pagination::{offset_page, validated_offset_params};
use crate::response::{finish_api_json, ok_json, ApiProblem};
use crate::state::LogQueryState;
use crate::tenant_scope::{require_tenant_read, resolve_list_tenant_id};
use axum::extract::{Path, Query, State};
use axum::response::Response;
use sdkwork_log_core::{LogApiSurface, RequestLogListQuery};
use sdkwork_web_core::WebRequestContext;

/// Lists request logs with optional filters (traceId, requestId, status,
/// apiSurface, operationId, time range) — offset pagination pushed to SQL.
pub async fn list_request_logs(
    ctx: WebRequestContext,
    State(state): State<LogQueryState>,
    Query(query): Query<AdminLogListQuery>,
) -> Response {
    finish_api_json(
        &ctx,
        async {
            require_tenant_read(&ctx)?;
            let tenant_id = resolve_list_tenant_id(&ctx, query.tenant_id.as_deref())?;
            if let Some(status) = query.status {
                if !(100..=599).contains(&status) {
                    return Err(ApiProblem::bad_request(
                        "status must be an HTTP status code in 100..599",
                    ));
                }
            }
            if let Some(min) = query.duration_min {
                if min < 0 {
                    return Err(ApiProblem::bad_request(
                        "duration_min must be >= 0",
                    ));
                }
            }
            if let Some(max) = query.duration_max {
                if max < 0 {
                    return Err(ApiProblem::bad_request(
                        "duration_max must be >= 0",
                    ));
                }
            }
            if let (Some(min), Some(max)) = (query.duration_min, query.duration_max) {
                if min > max {
                    return Err(ApiProblem::bad_request(
                        "duration_min must be <= duration_max",
                    ));
                }
            }
            let params = validated_offset_params(query.page, query.page_size, query.limit)
                .map_err(|code| ApiProblem::bad_request(code.title()))?;

            let mut store_query = RequestLogListQuery::new(params.page, params.page_size);
            store_query.tenant_id = tenant_id;
            store_query.trace_id = query.trace_id.filter(|value| !value.is_empty());
            store_query.request_id = query.request_id.filter(|value| !value.is_empty());
            store_query.api_surface = query
                .api_surface
                .filter(|value| !value.is_empty())
                .map(|value| LogApiSurface::parse(&value));
            store_query.method = query.method.filter(|value| !value.is_empty());
            store_query.operation_id = query.operation_id.filter(|value| !value.is_empty());
            store_query.service = query.service.filter(|value| !value.is_empty());
            store_query.status_code = query.status.map(|value| value as u16);
            store_query.duration_min = query.duration_min;
            store_query.duration_max = query.duration_max;
            store_query.created_from = query.created_from;
            store_query.created_to = query.created_to;

            let page = state.service.list_request_logs(store_query).await?;
            ok_json(offset_page(
                page.items
                    .into_iter()
                    .map(LogRequestListItem::from_row)
                    .collect(),
                page.total,
                params,
            ))
        }
        .await,
    )
}

/// Fetches one request log row by id, including the full redacted
/// request/response bodies. Tenant isolation mirrors list semantics: tenant
/// admins only ever see rows of their own tenant; a missing or foreign row is
/// reported as 404 so existence is not leaked.
pub async fn get_request_log(
    ctx: WebRequestContext,
    State(state): State<LogQueryState>,
    Path(id): Path<String>,
) -> Response {
    finish_api_json(
        &ctx,
        async {
            require_tenant_read(&ctx)?;
            let tenant_scope = resolve_list_tenant_id(&ctx, None)?;
            let row = state
                .service
                .get_request_log(&id)
                .await?
                .filter(|row| match &tenant_scope {
                    Some(scope) => row.record.tenant_id.as_deref() == Some(scope.as_str()),
                    None => true,
                })
                .ok_or_else(|| ApiProblem::not_found("request log was not found"))?;
            ok_json(LogRequestDetailEnvelope {
                item: LogRequestDetailItem::from_row(row),
            })
        }
        .await,
    )
}
