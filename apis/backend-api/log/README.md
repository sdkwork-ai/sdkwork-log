# LOG backend-api contract

Request log query surface (`log.requestLogs.list`). Generated artifacts:
`openapi.json` (OpenAPI authority) and `routes.manifest.json` (route manifest).

Refresh:

```bash
cargo test -p sdkwork-routes-log-backend-api materialize_openapi_authority_file -- --ignored
```
