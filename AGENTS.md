# Repository Guidelines

Domain: `platform`
Capability: `logging`
Type: **基础底层框架仓库**（非业务产品）
Status: `implementing`

SDKWork **独立请求日志基础模块**：框架无关的请求日志领域模型、持久化存储（`log_request`）、TTL 清理、查询契约，以及 web 框架捕获适配器。任何模块（web-framework、RPC、网关、定时任务）都可以集成本仓库的 crate 来记录和查询带 `traceId` 的请求日志。

## SDKWORK Soul

Read `../sdkwork-specs/SOUL.md` before executing repository tasks.

## SDKWORK Standards

Canonical entrypoint: `../sdkwork-specs/README.md`. Do not copy root standards into this repository.

## Application Identity

This repository is a platform foundation module, not an SDKWork application root. It must not contain product business routes.

## Local Dictionary Structure

- `AGENTS.md` — agent execution rules (this file).
- `specs/` — module component spec (`component.spec.json`).
- `crates/` — Rust crates: `sdkwork-log-core` (domain), `sdkwork-log-store-sqlx` (persistence), `sdkwork-log-database-host` (lifecycle), `sdkwork-log-web-adapter` (web-framework capture), `sdkwork-routes-log-backend-api` (query API).
- `database/` — database module (manifest, baseline DDL, contract registries).
- `apis/` — HTTP contract sources for the log backend-api surface.

## Spec Resolution Order

1. This `AGENTS.md`.
2. `specs/component.spec.json`.
3. `../sdkwork-specs/README.md` and task-specific root specs.
4. Implementation files.

## Required Specs By Task Type

| Task | Required specs |
| --- | --- |
| Agent/workflow rules | `../sdkwork-specs/SOUL.md`, `../sdkwork-specs/AGENTS_SPEC.md`, `../sdkwork-specs/SDKWORK_WORKSPACE_SPEC.md` |
| Any code change | `../sdkwork-specs/CODE_STYLE_SPEC.md`, `../sdkwork-specs/NAMING_SPEC.md`, `../sdkwork-specs/RUST_CODE_SPEC.md` |
| Log store / domain | `../sdkwork-specs/OBSERVABILITY_SPEC.md`, `../sdkwork-specs/DATABASE_SPEC.md` |
| Web adapter / routes | `../sdkwork-specs/WEB_FRAMEWORK_SPEC.md`, `../sdkwork-specs/API_SPEC.md` §10, `../sdkwork-specs/WEB_BACKEND_SPEC.md` |
| SQL store / migrations | `../sdkwork-specs/DATABASE_SPEC.md` |
| List/search API | `../sdkwork-specs/PAGINATION_SPEC.md` |
| Verification | `../sdkwork-specs/TEST_SPEC.md`, `../sdkwork-specs/QUALITY_GATE_SPEC.md` |

## 定位

- **是**：`RequestLogRecord` / `RequestLogStore` 领域契约、`log_request` 持久化与 TTL、web 捕获适配器、日志查询 API
- **不是**：IAM、电商、网关等业务；不包含 `sdkwork-routes-<业务>-*`
- **依赖**：`sdkwork-web-framework`（仅 `sdkwork-log-web-adapter` 与 `sdkwork-routes-log-backend-api`）、`sdkwork-database`（生命周期）、`sdkwork-utils`；`sdkwork-log-core` 保持框架无关

## Code Style Rules

Follow `../sdkwork-specs/RUST_CODE_SPEC.md`. `sdkwork-log-core` must not depend on web framework or database crates.

## Build, Test, and Verification

Canonical command list: `specs/component.spec.json` → `verification.commands`.

```bash
cargo test --workspace
cargo test -p sdkwork-log-store-sqlx
cargo test -p sdkwork-log-routes-log-backend-api 2>/dev/null || cargo test -p sdkwork-routes-log-backend-api
cargo test -p sdkwork-routes-log-backend-api --test routes_contract
cargo test -p sdkwork-routes-log-backend-api --test openapi_authority
cargo clippy --workspace -- -D warnings
```

## Agent Execution Rules

- Specs before memory; evidence before completion.
- Do not vendor framework pipeline source into this repository.
- `sdkwork-log-core` stays framework-agnostic: no `sdkwork-web-*` or `sqlx` dependencies.
- Do not hand-edit `apis/backend-api/log/openapi.json` or `routes.manifest.json`; regenerate through the `materialize_*` ignored tests.

## Human Review Rules

Human review is required for breaking standard changes, security exceptions, and changes to the `RequestLogStore` contract or the `log_request` table shape (consumers across repositories depend on them).
