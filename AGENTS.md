# Repository Guidelines

Domain: `platform`
Capability: `logging`
Type: **基础底层框架仓库**（非业务产品）
Status: `implementing`

SDKWork **独立请求日志基础模块**：框架无关的请求日志领域模型、持久化存储（`log_request`）、TTL 清理、查询契约，web 框架捕获适配器与 tower/axum 完整捕获中间件（含脱敏 body），以及可复用的前端面（PC / H5 / admin）与生成的 backend SDK。任何模块（web-framework、RPC、网关、定时任务）都可以集成本仓库的 crate 来记录和查询带 `traceId` 的请求日志；任何应用（如 sdkwork-cloudrouter）都可以通过 `@sdkwork/log-backend-sdk` 与 `@sdkwork/log-pc-admin-request-log` 集成管理端请求日志。

## SDKWORK Soul

Read `../sdkwork-specs/SOUL.md` before executing repository tasks.

## SDKWORK Standards

Canonical entrypoint: `../sdkwork-specs/README.md`. Do not copy root standards into this repository.

## Application Identity

This repository is a platform foundation module, not an SDKWork application root. It must not contain product business routes.

## Local Dictionary Structure

- `AGENTS.md` — agent execution rules (this file).
- `specs/` — module component spec (`component.spec.json`).
- `crates/` — Rust crates: `sdkwork-log-core` (domain), `sdkwork-log-store-sqlx` (persistence), `sdkwork-log-database-host` (lifecycle), `sdkwork-log-web-adapter` (web-framework capture), `sdkwork-log-tower-adapter` (tower/axum capture with redacted bodies), `sdkwork-routes-log-backend-api` (query API).
- `database/` — database module (manifest, baseline DDL, contract registries).
- `apis/` — HTTP contract sources for the log backend-api surface.
- `sdks/` — SDK family `sdkwork-log-backend-sdk` (generated TypeScript package `@sdkwork/log-backend-sdk`; regenerate via `pnpm sdk:generate:backend`).
- `apps/sdkwork-log-pc/packages/` — reusable PC surfaces: `sdkwork-log-pc-request-log` (types/service/components) and `sdkwork-log-pc-admin-request-log` (admin management page, surface `backend-admin`).
- `apps/sdkwork-log-h5/packages/` — reusable H5 surfaces: `sdkwork-log-h5-request-log`.
- `pnpm-workspace.yaml` — pnpm packages (SDK + frontend surfaces).

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
| Frontend surfaces (pc/h5/admin) | `../sdkwork-specs/TYPESCRIPT_CODE_SPEC.md`, `../sdkwork-specs/FRONTEND_CODE_SPEC.md`, `../sdkwork-specs/BACKEND_UI_SPEC.md` |
| SDK regeneration | `../sdkwork-specs/SDK_SPEC.md`, `../sdkwork-specs/SDK_WORKSPACE_GENERATION_SPEC.md` |
| Verification | `../sdkwork-specs/TEST_SPEC.md`, `../sdkwork-specs/QUALITY_GATE_SPEC.md` |

## 定位

- **是**：`RequestLogRecord` / `RequestLogStore` 领域契约（含脱敏后的完整请求/响应体）、`log_request` 持久化与 TTL、web 捕获适配器与 tower/axum 捕获中间件、日志查询 API（list + detail）、生成的 `@sdkwork/log-backend-sdk`、可复用前端面（PC / H5 / admin）
- **不是**：IAM、电商、网关等业务；不包含 `sdkwork-routes-<业务>-*`
- **依赖**：`sdkwork-web-framework`（仅 `sdkwork-log-web-adapter` 与 `sdkwork-routes-log-backend-api`）、`sdkwork-database`（生命周期）、`sdkwork-utils`、`sdkwork-sdk-generator`（SDK 生成）；`sdkwork-log-core` 保持框架无关

## Int64 Wire Contract (API_SPEC §13.6)

- OpenAPI `int64` fields and parameters `MUST` be `type: string`, `format: int64`,
  a decimal `pattern` such as `^-?[0-9]+$`, and `x-sdkwork-int64-string: true`.
  `type: integer, format: int64` is a contract violation: generated TypeScript
  SDKs then emit `number`, and browsers silently round ids past
  `Number.MAX_SAFE_INTEGER` (2^53), replaying wrong ids into lookups.
- Rust response DTOs `MUST` serialize `i64` wire fields with
  `#[serde(with = "sdkwork_utils_rust::serde_int64")]` (or `::option`); request
  boundaries parse inbound strings with the same helper.
- Generated TypeScript SDKs keep `int64` as `string`; frontend code `MUST NOT`
  convert ids/snowflake ids/sequence ids to `number` for storage, comparison,
  or submission.
- Verification: `node <sdkwork-specs>/tools/check-api-operation-patterns.mjs --workspace .`

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
pnpm install
pnpm typecheck
```

## Agent Execution Rules

- Specs before memory; evidence before completion.
- Do not vendor framework pipeline source into this repository.
- `sdkwork-log-core` stays framework-agnostic: no `sdkwork-web-*` or `sqlx` dependencies.
- Do not hand-edit `apis/backend-api/log/openapi.json` or `routes.manifest.json`; regenerate through the `materialize_*` ignored tests.
- Do not hand-edit generated SDK output (`sdks/sdkwork-log-backend-sdk/.../generated/`); regenerate via `pnpm sdk:generate:backend` (input: the committed OpenAPI authority).
- Body capture stores **redacted text only** (`DATABASE_SPEC.md` §18): sensitive field values are replaced with `[REDACTED]` before persistence.

## Human Review Rules

Human review is required for breaking standard changes, security exceptions, and changes to the `RequestLogStore` contract or the `log_request` table shape (consumers across repositories depend on them).

<!-- SDKWORK-NAMING-STANDARD: v1 -->
## Rust Naming And Dependency Declaration

Authority: `../sdkwork-specs/NAMING_SPEC.md` section 3.1 and section 3.2.

Two identifier planes exist in every Rust crate and they MUST NOT be mixed: the package plane
(Cargo, filesystem, lock file) uses kebab-case, and the crate plane (lib target, modules, source
imports) uses snake_case.

- `[package].name`, the crate directory, `[features]` keys, and `[[bin]].name` use kebab-case.
- `[lib].name`, module files, module directories, and Rust imports use snake_case.
- A crate whose `[package].name` contains a hyphen SHOULD declare `[lib].name` explicitly
  (default: package name with every `-` replaced by `_`). A shorter lib name is allowed only
  when declared explicitly and used consistently by every consumer.
- Cargo dependency keys, `[workspace.dependencies]` keys, and `Cargo.lock` entries use the
  dependency package name. Use `package = "..."` when an alias is required.
- Every external crate referenced by `src/` MUST be declared in that crate's `[dependencies]`.
  Test-only crates belong in `[dev-dependencies]`; `build.rs` crates belong in
  `[build-dependencies]`.
- Never delete a dependency line, and never demote one from `[dependencies]` to
  `[dev-dependencies]`, while `src/` still imports it. Verify manifest cleanups with the
  command below before committing them.
- Regenerate and commit `Cargo.lock` in the same change as any dependency table edit.

Verification:

```bash
node ../sdkwork-specs/tools/check-rust-crate-naming-standard.mjs --root .
```
<!-- /SDKWORK-NAMING-STANDARD: v1 -->

<!-- SDKWORK-RUST-CODE-STANDARD: v1 -->
## Rust Code Standard

Authority: `../sdkwork-specs/RUST_CODE_SPEC.md` (v2, industry-best baseline); package/crate
naming and dependency declaration are normative in `../sdkwork-specs/NAMING_SPEC.md` section 3.1
and 3.2.

- Crates are responsibility-shaped: service, repository-sqlx, routes, service-host, native-host,
  worker, assembly, gateway. No generic `core`/`common`/`backend`/`runtime` suffixes.
- Errors are typed enums (`thiserror`) implementing `std::error::Error` with a `source` chain.
  `anyhow` only at binary/CLI/test boundaries, never in lib `[dependencies]`.
- No `unsafe` without a `// SAFETY:` comment; crates default to `unsafe_code = "forbid"`.
  No `unwrap`/`expect`/`panic!`/`todo!`/`dbg!` in library code reachable from public API.
- No lock guard held across `.await`; every external await has a timeout; spawned tasks are
  awaited/detached with a documented owner; retries are bounded, jittered, and idempotent.
- Public API is minimal, documented, `#[must_use]` where applicable, and semver-clean. Leaking
  framework types (`sqlx::Row`, axum extractors) through public signatures is forbidden.
- Workspace root declares `[workspace.package]` (edition, rust-version) and `[workspace.lints]`
  (RUST_CODE_SPEC.md section 13 baseline); every member inherits both with
  `edition.workspace = true` and `[lints] workspace = true`.

Verification:

```bash
node ../sdkwork-specs/tools/check-rust-crate-naming-standard.mjs --root .
node ../sdkwork-specs/tools/check-rust-manifest-standard.mjs --root .
# when service/repository/route/gateway dependencies change:
node ../sdkwork-specs/tools/check-rust-backend-composition.mjs --root .
```
<!-- /SDKWORK-RUST-CODE-STANDARD: v1 -->

<!-- SDKWORK-TYPESCRIPT-CODE-STANDARD: v1 -->
## TypeScript Code Standard

Authority: `../sdkwork-specs/TYPESCRIPT_CODE_SPEC.md` (v2, industry-best baseline).

- `tsconfig` runs `strict: true` and the strict family; public APIs are typed and `any`-free.
  `import type` is required for type-only imports (`verbatimModuleSyntax`).
- Errors are typed at package/service boundaries; no empty catches, no swallowed promise
  rejections, no bare `throw new Error('...')` for business failures.
- Async: every promise is settled; external awaits have timeouts; `AbortSignal` accepted for
  cancellable work; bounded concurrency; no unbounded `Promise.all`.
- Public API is minimal, JSDoc-documented, `@deprecated` where applicable, and semver-clean.
- Discriminated unions model closed variant sets; no `as`/`@ts-ignore` bypasses without a guard.
- Node/build runners verify build-critical sources and self-heal from git (CODE_STYLE_SPEC §7);
  `pnpm clean` never deletes git-tracked build-critical files.

Verification:

```bash
pnpm typecheck && pnpm test && pnpm lint
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
```
<!-- /SDKWORK-TYPESCRIPT-CODE-STANDARD: v1 -->

<!-- SDKWORK-FRONTEND-CODE-STANDARD: v1 -->
## Frontend Code Standard

Authority: `../sdkwork-specs/FRONTEND_CODE_SPEC.md` (v2); language rules follow
`../sdkwork-specs/TYPESCRIPT_CODE_SPEC.md` (React/TS) or `../sdkwork-specs/DART_CODE_SPEC.md` (Flutter).

- UI -> service -> injected SDK flow is preserved; components never construct SDK clients or
  assemble raw HTTP/auth headers.
- React: hooks rules clean (`react-hooks`), `useEffect` with full deps and cleanup, stable
  list keys, error boundaries at route/page level, derived state during render (not in effects).
- State: server state behind services/query layer; client state local or minimal typed store;
  no duplication of server state in client stores.
- Accessibility: accessible names, keyboard behavior, visible focus, color is never the only
  signal; error states announced.
- i18n for all user-facing copy in reusable/user-facing packages (I18N_SPEC §6.1).
- PC/H5 `outDir` uses `dist/{standalone,cloud}/{dev,test,staging,prod}`.

Verification:

```bash
pnpm typecheck && pnpm test && pnpm lint
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
node ../sdkwork-specs/tools/check-browser-dist-layout.mjs --root .   # PC/H5 apps
```
<!-- /SDKWORK-FRONTEND-CODE-STANDARD: v1 -->

<!-- SDKWORK-PNPM-WORKSPACE-STANDARD: v1 -->
## pnpm Workspace Dependency And Package Import

Authority: `../sdkwork-specs/PNPM_WORKSPACE_DEPENDENCY_SPEC.md` (companion to
`../sdkwork-specs/DEPENDENCY_MANAGEMENT_SPEC.md`).

Sibling SDKWork repositories are consumed through a dual-track model that MUST stay consistent:

- **Local development** (`pnpm dev`, `pnpm build`): pnpm workspace protocol. Each sibling
  package is declared ONCE in this repository root `pnpm-workspace.yaml` `packages:` as a
  `../sdkwork-*` relative path, and consumed with `workspace:*` in `package.json`. Never use
  `file:`/`link:`/git-URL specifiers for SDKWork sibling packages in any environment.
- **CI / release packaging**: git-repository dependency checkout. Every sibling referenced by the
  local workspace MUST have a matching `dependencies[]` entry in `sdkwork.workflow.json` so CI
  clones the sibling into the same `../sdkwork-*` relative layout (`GITHUB_WORKFLOW_SPEC.md`).
  `package.json` is never rewritten for CI.

Import rules for sibling SDKWork packages:

- Import by package name only: `import { X } from "@sdkwork/package-name"`. The specifier MUST
  equal the target package's `package.json` `name` exactly - no shortening, renaming, or alias.
- Forbidden: relative imports that cross a package boundary into another SDKWork repository or
  another workspace package's `src/` (for example `import ... from "../../sdkwork-appbase/.../src/..."`).
- Consume only the public `exports` surface of a package; never deep-import sibling `src/` internals.
- Every non-relative import in a workspace member MUST resolve to that member's own
  `dependencies`/`devDependencies`/`peerDependencies` (import closure).
- Vite aliases MUST NOT rename or redirect `@sdkwork/*` packages, MUST NOT be added to make a
  resolution error pass, and are allowed only for documented bootstrap/SDK-generation entrypoints.
- Fix a resolution failure by correcting the workspace declaration or the package `exports`,
  not by adding an alias.

Verification:

```bash
node ../sdkwork-specs/tools/verify-repo.mjs --root .
node ../sdkwork-specs/tools/check-workspace-member-protocol.mjs --root .
node ../sdkwork-specs/tools/check-dependency-list-completeness.mjs --target <repo-name>
```
<!-- /SDKWORK-PNPM-WORKSPACE-STANDARD: v1 -->
