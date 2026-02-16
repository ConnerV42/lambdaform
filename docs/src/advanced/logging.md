# Structured Logging

Lambdaform supports JSON log output for integration with log aggregators.

## Enabling JSON Logs

```bash
lambdaform start --json-log
```

Or in `lambdaform.yaml`:

```yaml
json_log: true
```

## Log Format

Each request produces a structured JSON line:

```json
{
  "timestamp": "2024-01-15T18:30:45.123Z",
  "level": "info",
  "event": "request",
  "method": "GET",
  "path": "/users/123",
  "status": 200,
  "duration_ms": 3.2,
  "function": "get_user",
  "gateway": "api",
  "request_id": "local-a1b2c3d4"
}
```

## Fields

| Field | Description |
|-------|-------------|
| `timestamp` | ISO 8601 timestamp |
| `level` | `info`, `warn`, `error` |
| `event` | `request`, `startup`, `reload`, `error` |
| `method` | HTTP method |
| `path` | Request path |
| `status` | HTTP status code |
| `duration_ms` | Response time in milliseconds |
| `function` | Lambda function name that handled the request |
| `gateway` | API Gateway resource name |
| `request_id` | Generated request ID |

## Use Cases

- **Log aggregation:** Pipe to Datadog, CloudWatch, or ELK
- **Monitoring:** Parse JSON logs for latency metrics
- **CI/CD:** Machine-readable output for test assertions
- **Debugging:** `jq` filtering on structured fields

```bash
# Filter slow requests
lambdaform start --json-log 2>&1 | jq 'select(.duration_ms > 100)'

# Count requests per function
lambdaform start --json-log 2>&1 | jq -s 'group_by(.function) | map({function: .[0].function, count: length})'
```
