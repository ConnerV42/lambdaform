# API Gateway v2 (HTTP API) Example

Demonstrates API Gateway v2 (HTTP API) with Lambdaform, using the v2 event format (`version: "2.0"`).

## Features Tested

- `aws_apigatewayv2_api` with `protocol_type = "HTTP"`
- `aws_apigatewayv2_route` with route keys (`GET /items`, `POST /items`, etc.)
- `aws_apigatewayv2_integration` with `payload_format_version = "2.0"`
- **Multi-function routing** — CRUD routes → `api` function, health check → `health` function
- **v2 event format** — `version`, `routeKey`, `rawPath`, `requestContext.http`
- **CORS configuration** via `cors_configuration` block
- **Path parameters** — `{id}` in route keys

## Running

```bash
lambdaform start
```

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | /items | List all items |
| POST | /items | Create item |
| GET | /items/{id} | Get item by ID |
| PUT | /items/{id} | Update item |
| DELETE | /items/{id} | Delete item |
| GET | /health | Health check (separate function) |

## v2 vs v1 Differences

The v2 event format includes:
- `event.version` = `"2.0"`
- `event.routeKey` = `"GET /items"` (method + path)
- `event.rawPath` = `/items`
- `event.requestContext.http.method` (instead of `event.httpMethod`)
- `event.requestContext.http.path` (instead of `event.path`)
- Cookies as a separate array (not in headers)
