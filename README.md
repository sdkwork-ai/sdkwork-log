# sdkwork-log

SDKWork **独立请求日志基础模块**（platform / logging）。框架无关的请求日志领域模型、`log_request` 持久化存储（SQLite / PostgreSQL）、TTL 清理与查询契约，以及 web 框架捕获适配器与查询 API。

## 定位

- `sdkwork-log-core`：`RequestLogRecord` / `RequestLogStore` 领域契约与捕获脱敏工具 —— **不依赖任何 web 框架或数据库 crate**，可被任意传输层（HTTP web 框架、RPC、网关、定时任务）集成。
- `sdkwork-log-store-sqlx`：SQLx 实现（`log_request` 表、save / 分页 list、TTL 清理），SQLite + PostgreSQL 双引擎。
- `sdkwork-log-database-host`：走 `sdkwork-database` 生命周期（baseline + migrations）的数据库主机（`sdkwork-log-db` CLI）。
- `sdkwork-log-web-adapter`：`sdkwork-web-framework` 捕获适配器 —— 追加到标准拦截器链（EP-14，不改 18 阶段顺序），**覆盖所有经框架的 HTTP 请求（含 webhook、各 API surface、WS 升级）**，持久化每条请求的 `traceId`、状态、耗时、请求参数等。
- `sdkwork-routes-log-backend-api`：请求日志查询 API（列表/搜索、分页、按 `traceId` 过滤）。

## 记录内容（一条请求一行）

| 字段 | 来源 | 说明 |
| --- | --- | --- |
| `trace_id` | 框架 ResponseIdentity 同源解析（traceparent → trace id，requestId 兜底） | REQUIRED（OBSERVABILITY §2） |
| `request_id` | 框架阶段 1 | 与审计行关联 |
| `tenant_id` / `user_id` | 解析后的 principal | 安全租户上下文（SECURITY §6） |
| `api_surface` | 框架分类 | open-api / app-api / backend-api / internal-api / gateway-api |
| `path` | 路由模板（脱敏） | 禁止原始路径（OBSERVABILITY §2） |
| `method` / `operation_id` | 框架 | — |
| `service` | 接线方 `with_service(...)` 指定 | SHOULD（OBSERVABILITY §2） |
| `environment` | 框架 profile | dev / test / prod |
| `auth_mode` | 框架认证模式 | public / api-key / dual-token / ... |
| `status_code` / `duration_ms` | 响应 + 阶段 15 计时 | — |
| `error_code` / `failed_stage` | before 阶段失败 | 平台数值码 + 拦截器阶段 |
| `query_params` | before 阶段捕获 | **敏感 key 值替换为 `[REDACTED]`** |
| `request_headers` | before 阶段捕获 | 白名单安全头 JSON（不含任何凭证/cookie） |
| `created_at` / `expires_at` | 存储层 | epoch 秒；TTL 默认 90 天可配置 |

**不记录**：请求/响应体、Authorization/API-Key/Cookie 等凭证头、referer（可携带签名 URL）—— `OBSERVABILITY_SPEC.md` §2 / `DATABASE_SPEC.md` §18 禁止存储敏感值。

## 快速接入（web 服务）

```rust
use std::sync::Arc;
use sdkwork_log_core::RequestLogStore;
use sdkwork_log_store_sqlx::SqlxRequestLogStore;
use sdkwork_log_web_adapter::RequestLoggingInterceptor;
use sdkwork_web_core::WebCallInterceptorChain;

// 1. 建 store（SQLite 示例；Postgres 用 new_postgres / new）
let store: Arc<dyn RequestLogStore> =
    Arc::new(SqlxRequestLogStore::new_sqlite(pool));

// 2. 追加捕获拦截器（不改变 18 阶段顺序；with_service 记录服务名）
let chain = WebCallInterceptorChain::standard()
    .with_interceptor(
        RequestLoggingInterceptor::new(store.clone())
            .with_service("sdkwork-api-iam-assembly"),
    );

// 3. 装配进框架
let framework = WebFramework::builder(resolver)
    .route_manifest(manifest)
    .call_chain(chain)
    .build();

// 4. 挂载查询 API（GET /backend/v3/api/log/request_logs）
let router = sdkwork_routes_log_backend_api::build_router(store);
```

查询过滤：`trace_id` / `request_id` / `tenant_id` / `api_surface` / `operation_id` / `status` / `created_from` / `created_to`，offset 分页（默认 20、上限 200）。

## 规范对齐（sdkwork-specs）

| 条款 | 要求 | 实现 |
| --- | --- | --- |
| OBSERVABILITY §1 | 每 API 请求 SHOULD 有 traceId | `trace_id NOT NULL`，框架同源解析 |
| OBSERVABILITY §2 | access log MUST 用 traceId（禁 requestId） | 主关联字段即 `trace_id` |
| OBSERVABILITY §2 | 字段 SHOULD 含 service/environment/stage/auth/status/duration/租户 | 全部落列 |
| OBSERVABILITY §2 | 禁存 token/密钥/敏感 payload | 捕获时脱敏 + 头白名单 + 不存 body |
| OBSERVABILITY §2 | 路由 MUST 用模板 | `redact_path_template` 同源 |
| DATABASE §6.5 / §18 | trace_id 审计字段；敏感值禁存 | 同上 |
| SECURITY §6 | 日志含关联 id 与安全租户上下文 | `trace_id` + `tenant_id` |
| API §14.1/§15/§16 | `SdkWorkListQuery` 分页、`SdkWorkApiResponse` 信封、`ProblemDetail` | 查询 API 遵循 |
| PAGINATION | offset 分页默认 20 / 上限 200 / SQL 层 LIMIT | store 实现 |

## 验证

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo test -p sdkwork-routes-log-backend-api --test routes_contract
cargo test -p sdkwork-routes-log-backend-api --test openapi_authority
```
