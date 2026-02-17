# Plugin Architecture

Lambdaform's plugin system lets you extend the emulator with custom resource handlers — local S3 emulation, custom auth providers, mock services, and more — without modifying Lambdaform's core code.

## How It Works

Plugins are **external executables** (any language) that communicate with Lambdaform via **JSON over stdin/stdout**. Lambdaform calls your plugin at specific lifecycle hooks:

| Hook | When | Can Modify |
|------|------|------------|
| `describe` | Startup (once) | — |
| `on_resource` | Terraform resource parsed | Side effects (env vars, endpoints) |
| `on_request` | Before Lambda invocation | The Lambda event |
| `on_response` | After Lambda returns | The response |

## Configuration

Add plugins to your `lambdaform.yaml`:

```yaml
plugins:
  - name: s3-local
    path: ./plugins/s3-local.py
    config:
      data_dir: /tmp/s3-data
      
  - name: auth-mock
    path: /usr/local/bin/lambdaform-auth-mock
```

- **`name`**: Identifier used in logs
- **`path`**: Absolute or relative to project root
- **`config`**: Arbitrary key-value pairs passed to the plugin

## Plugin Protocol

### Request Format

Lambdaform sends a JSON object to the plugin's stdin with a `kind` field:

```json
{
  "kind": "describe",
  "config": { "data_dir": "/tmp/s3-data" }
}
```

### Response Format

The plugin writes a JSON response to stdout:

```json
{
  "ok": true,
  "capabilities": {
    "version": "1.0.0",
    "resource_types": ["aws_s3_bucket", "aws_s3_object"],
    "intercept_requests": false,
    "intercept_responses": false,
    "description": "Local S3 emulation"
  }
}
```

### The `describe` Handshake

Called once at startup. Your plugin must return its capabilities:

- **`resource_types`**: Terraform resource types you handle (e.g., `["aws_s3_bucket"]`)
- **`intercept_requests`**: Set `true` to receive `on_request` hooks
- **`intercept_responses`**: Set `true` to receive `on_response` hooks

### The `on_resource` Hook

Called when Lambdaform parses a Terraform resource matching your `resource_types`:

```json
{
  "kind": "on_resource",
  "resource_type": "aws_s3_bucket",
  "resource_name": "uploads",
  "attributes": { "bucket": "my-uploads-bucket" },
  "config": { "data_dir": "/tmp/s3-data" }
}
```

Respond with optional **side effects**:

```json
{
  "ok": true,
  "side_effects": [
    {
      "kind": "env_var",
      "functions": [],
      "key": "S3_ENDPOINT",
      "value": "http://localhost:9000"
    },
    {
      "kind": "endpoint",
      "service": "s3",
      "url": "http://localhost:9000"
    },
    {
      "kind": "log",
      "level": "info",
      "message": "Created local bucket: my-uploads-bucket"
    }
  ]
}
```

Side effect types:
- **`env_var`**: Inject environment variables into Lambda functions (`functions: []` = all functions)
- **`endpoint`**: Register a local service endpoint
- **`log`**: Emit a log message through Lambdaform's logger

### The `on_request` Hook

Called before each Lambda invocation (if `intercept_requests: true`):

```json
{
  "kind": "on_request",
  "method": "POST",
  "path": "/api/upload",
  "event": { "httpMethod": "POST", "body": "..." },
  "function_name": "upload_handler",
  "config": {}
}
```

Return a modified event (or omit `event` to pass through unchanged):

```json
{
  "ok": true,
  "event": { "httpMethod": "POST", "body": "...", "injected_field": "value" }
}
```

### The `on_response` Hook

Called after each Lambda invocation (if `intercept_responses: true`):

```json
{
  "kind": "on_response",
  "method": "POST",
  "path": "/api/upload",
  "response": { "statusCode": 200, "body": "..." },
  "function_name": "upload_handler",
  "config": {}
}
```

## Writing a Plugin

Here's a minimal Python plugin that logs S3 bucket creation:

```python
#!/usr/bin/env python3
"""Example Lambdaform plugin: S3 bucket logger."""
import json, sys

request = json.loads(sys.stdin.read())

if request["kind"] == "describe":
    print(json.dumps({
        "ok": True,
        "capabilities": {
            "version": "0.1.0",
            "resource_types": ["aws_s3_bucket"],
            "intercept_requests": False,
            "intercept_responses": False,
            "description": "Logs S3 bucket resources"
        }
    }))

elif request["kind"] == "on_resource":
    bucket = request["attributes"].get("bucket", "unknown")
    print(json.dumps({
        "ok": True,
        "side_effects": [
            {"kind": "log", "level": "info", "message": f"Found S3 bucket: {bucket}"}
        ]
    }))

else:
    print(json.dumps({"ok": True}))
```

Make it executable (`chmod +x`) and reference it in `lambdaform.yaml`.

## Listing Plugins

```bash
lambdaform plugins --dir .
```

Shows all configured plugins and their capabilities.

## Error Handling

- If a plugin **fails to start** or **times out** (10s default), Lambdaform reports the error and aborts startup.
- If a plugin returns `"ok": false` during `on_resource`/`on_request`/`on_response`, Lambdaform logs a warning and continues (non-fatal).
- Plugin stderr is captured and logged at debug level.

## Use Cases

- **Local S3**: Create directories for buckets, serve files via a local HTTP server
- **DynamoDB Local**: Auto-start DynamoDB Local, inject endpoint env vars
- **Auth mocking**: Inject test JWT tokens or mock Cognito responses
- **Request logging**: Custom analytics or debugging middleware
- **Service virtualization**: Mock external APIs your Lambdas depend on
