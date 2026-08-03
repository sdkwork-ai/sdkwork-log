# API contracts

HTTP contract sources for the log foundation's backend-api surface. Materialization
and SDK generation follow `API_SPEC.md` and `SDK_WORKSPACE_GENERATION_SPEC.md`.

| Surface | Path | Owner |
| --- | --- | --- |
| backend-api | `backend-api/log/` | `sdkwork-routes-log-backend-api` |

`openapi.json` and `routes.manifest.json` are generated from the Rust route
manifest — never hand-edit; refresh via:

```bash
cargo test -p sdkwork-routes-log-backend-api materialize_openapi_authority_file -- --ignored
```
