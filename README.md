# sdkwork-log

SDKWork **独立请求日志基础模块**（platform / logging）。框架无关的请求日志领域模型、`log_request` 持久化存储（SQLite / PostgreSQL）、TTL 清理与查询契约、web 框架捕获适配器与 tower/axum 完整捕获中间件（含脱敏 body），以及可复用前端面与生成的 backend SDK。

## 定位

- `sdkwork-log-core`：`RequestLogRecord` / `RequestLogStore` 领域契约与捕获脱敏工具 —— **不依赖任何 web 框架或数据库 crate**，可被任意传输层（HTTP web 框架、RPC、网关、定时任务）集成。
- `sdkwork-log-store-sqlx`：SQLx 实现（`log_request` 表、save / 分页 list / detail get、TTL 清理），SQLite + PostgreSQL 双引擎。
- `sdkwork-log-database-host`：走 `sdkwork-database` 生命周期（baseline + migrations）的数据库主机（`sdkwork-log-db` CLI）。
- `sdkwork-log-web-adapter`：`sdkwork-web-framework` 捕获适配器 —— 追加到标准拦截器链（EP-14，不改 18 阶段顺序），**覆盖所有经框架的 HTTP 请求（含 webhook、各 API surface、WS 升级）**，持久化每条请求的 `traceId`、状态、耗时、请求参数等（元数据；框架不向拦截器暴露 body）。
- `sdkwork-log-tower-adapter`：tower/axum 捕获中间件 —— 自包含完整记录（元数据 + **完整脱敏后的请求/响应体**），适合 axum 应用（如 sdkwork-cloudrouter）。请求/响应体按 `DATABASE_SPEC.md` §18 脱敏后落库，敏感字段值替换为 `[REDACTED]`。
- `sdkwork-routes-log-backend-api`：请求日志查询 API（列表/搜索 + 详情，分页，traceId 等过滤）。
- `sdks/sdkwork-log-backend-sdk`：生成的 `@sdkwork/log-backend-sdk` TypeScript 包（`pnpm sdk:generate:backend` 再生成）。
- `apps/sdkwork-log-pc/packages/`：可复用 PC 面 —— `@sdkwork/log-pc-request-log`（类型/服务/组件）、`@sdkwork/log-pc-admin-request-log`（管理端页面，surface `backend-admin`，含 i18n）。
- `apps/sdkwork-log-h5/packages/`：可复用 H5 面 —— `@sdkwork/log-h5-request-log`。

## 记录内容（一条请求一行）

| 字段 | 来源 | 说明 |
| --- | --- | --- |
| `trace_id` | traceparent → trace id（`x-sdkwork-trace-id` / `x-request-id` 兜底） | REQUIRED（OBSERVABILITY §2） |
| `request_id` | `x-request-id`（兜底 trace_id） | 与审计行关联 |
| `tenant_id` / `user_id` | 接线方 resolver（tower 适配器）或框架 principal（web 适配器） | 安全租户上下文（SECURITY §6） |
| `api_surface` | 路径前缀推断（tower）或框架分类（web） | open-api / app-api / backend-api / internal-api / gateway-api |
| `path` | 路由模板（可接线 resolver 提供；默认原始路径） | 禁止原始路径（OBSERVABILITY §2，web 适配器用模板） |
| `method` / `operation_id` | 请求 / 接线方 | — |
| `service` | 接线方 `with_service(...)` 指定 | SHOULD（OBSERVABILITY §2） |
| `environment` | 接线方 | dev / test / prod |
| `auth_mode` | 接线方 | public / api-key / dual-token / ... |
| `status_code` / `duration_ms` | 响应 + 计时 | — |
| `error_code` / `failed_stage` | 请求失败时 | 平台数值码 + 阶段 |
| `query_params` | 捕获 | **敏感 key 值替换为 `[REDACTED]`** |
| `request_headers` | 捕获 | 白名单安全头 JSON（不含任何凭证/cookie） |
| `request_body` / `response_body` | tower 适配器捕获（限长 256 KiB） | **完整文本 + 敏感字段值 `[REDACTED]`**；二进制体不存储 |
| `created_at` / `expires_at` | 存储层 | epoch 秒；TTL 默认 90 天可配置 |

**脱敏策略**：JSON 体按结构递归脱敏（敏感 key 的值替换为 `[REDACTED]`，形状保留）；非 JSON 文本按行保守替换。`Authorization`/`API-Key`/`Cookie` 等凭证头永不存储（`OBSERVABILITY_SPEC.md` §2 / `DATABASE_SPEC.md` §18）。列表接口不返回 body（大 payload 不进列表），详情接口返回完整输入输出。

## 快速接入（axum / tower 应用）

```rust,ignore
use std::sync::Arc;
use sdkwork_log_core::RequestLogStore;
use sdkwork_log_store_sqlx::SqlxRequestLogStore;
use sdkwork_log_tower_adapter::RequestLoggingLayer;

// 1. 建 store（Postgres 示例；SQLite 用 new_sqlite）
let store: Arc<dyn RequestLogStore> =
    Arc::new(SqlxRequestLogStore::new_postgres(pool));

// 2. 包裹 axum Router（完整记录：元数据 + 脱敏请求/响应体）
let app = Router::new()
    .route("/backend/v3/api/system/records", get(records))
    .layer(
        RequestLoggingLayer::new(store.clone())
            .with_service("sdkwork-cloudrouter"),
    );

// 3. 挂载查询 API（GET /backend/v3/api/log/request_logs + /{id}）
let router = sdkwork_routes_log_backend_api::build_router(store);
let app = app.merge(router);
```

查询过滤：`trace_id` / `request_id` / `tenant_id` / `api_surface` / `operation_id` / `service` / `status` / `created_from` / `created_to`，offset 分页（默认 20、上限 200）。详情：`GET /backend/v3/api/log/request_logs/{id}` 返回含 `request_body` / `response_body` 的完整记录。

## 前端集成（sdkwork-cloudrouter 等）

```bash
pnpm add @sdkwork/log-pc-admin-request-log @sdkwork/log-pc-request-log @sdkwork/log-backend-sdk
```

- 管理端页面：`@sdkwork/log-pc-admin-request-log`（`RequestLogAdmin`，列表 + 详情抽屉，i18n 见 `./i18n`）。
- PC / H5 应用面：`@sdkwork/log-pc-request-log` / `@sdkwork/log-h5-request-log`（类型、`RequestLogService`、展示组件）。
- 客户端配置：`configureLogBackendSdkClient({ baseUrl, tokenManager, platform })` 在应用启动时注入一次。

## 规范对齐（sdkwork-specs）

| 条款 | 要求 | 实现 |
| --- | --- | --- |
| OBSERVABILITY §1 | 每 API 请求 SHOULD 有 traceId | `trace_id NOT NULL`，同源解析 |
| OBSERVABILITY §2 | access log MUST 用 traceId（禁 requestId） | 主关联字段即 `trace_id` |
| OBSERVABILITY §2 | 字段 SHOULD 含 service/environment/stage/auth/status/duration/租户 | 全部落列 |
| OBSERVABILITY §2 | 禁存 token/密钥/敏感 payload | 捕获时脱敏 + 头白名单 + body 脱敏存储 |
| OBSERVABILITY §2 | 路由 MUST 用模板 | web 适配器用模板；tower 适配器可接线模板 resolver |
| DATABASE §6.5 / §18 | trace_id 审计字段；敏感值禁存 | 同上 |
| SECURITY §6 | 日志含关联 id 与安全租户上下文 | `trace_id` + `tenant_id` |
| API §14.1/§15/§16 | `SdkWorkListQuery` 分页、`SdkWorkApiResponse` 信封、`ProblemDetail` | 查询 API 遵循 |
| PAGINATION | offset 分页默认 20 / 上限 200 / SQL 层 LIMIT | store 实现 |
| BACKEND_UI | pc-admin 段包声明 `surface: backend-admin` | `sdkwork-log-pc-admin-request-log` |

## 验证

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo test -p sdkwork-routes-log-backend-api --test routes_contract
cargo test -p sdkwork-routes-log-backend-api --test openapi_authority
pnpm install
pnpm typecheck
```
