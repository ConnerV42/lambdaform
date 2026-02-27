# Troubleshooting

Common issues and how to resolve them.

## Startup Issues

### "No Lambda functions found"

Lambdaform reads `aws_lambda_function` resources from `.tf` files. Check that:

1. You're running `lambdaform start` in the directory containing your `.tf` files
2. Your Lambda functions use the standard `aws_lambda_function` resource type
3. The `runtime` attribute is set (e.g., `nodejs20.x`, `python3.12`)
4. The `handler` attribute follows the `file.function` format

```hcl
# ✅ This works
resource "aws_lambda_function" "hello" {
  function_name = "hello"
  runtime       = "nodejs20.x"
  handler       = "index.handler"
  filename      = "lambda.zip"
}

# ❌ Missing runtime — Lambdaform can't determine how to invoke
resource "aws_lambda_function" "hello" {
  function_name = "hello"
  handler       = "index.handler"
  filename      = "lambda.zip"
}
```

### "Failed to spawn Node.js/Python worker"

The runtime binary isn't found on your PATH. Lambdaform requires:

- **Node.js runtimes** (`nodejs18.x`, `nodejs20.x`, `nodejs22.x`): `node` binary
- **Python runtimes** (`python3.10`–`python3.13`): `python3` binary
- **Go runtime** (`go1.x`, `provided.al2`): `go` binary (for building)
- **Rust runtime** (`provided.al2023`): `cargo` binary (for building)
- **Java runtimes** (`java8.al2`–`java21`): Docker (uses AWS Lambda base images)

Verify your runtime is installed:

```bash
node --version    # Should print v18+ for nodejs18.x
python3 --version # Should print 3.10+ for python3.10
```

### "Worker failed to start within 30s"

The Lambda function has an import-time side effect that blocks startup. Common causes:

- Network calls during module import (e.g., connecting to a database at import time)
- Heavy computation in module-level code
- Missing dependencies (check stderr output for import errors)

**Fix:** Move initialization code inside the handler function, or use lazy initialization.

### Terraform/OpenTofu not found warning

Lambdaform doesn't require Terraform or OpenTofu to run — it parses `.tf` files directly. The warning just means you won't be able to run `terraform init/plan/apply` for deployment. You can safely ignore it for local development.

## API Gateway Issues

### Routes return 404

Check that your Terraform config has the full API Gateway chain:

**REST API (v1):**
```hcl
resource "aws_api_gateway_rest_api" "api" { ... }
resource "aws_api_gateway_resource" "items" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "items"
}
resource "aws_api_gateway_method" "get_items" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  resource_id = aws_api_gateway_resource.items.id
  http_method = "GET"
  authorization = "NONE"
}
resource "aws_api_gateway_integration" "get_items" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  resource_id = aws_api_gateway_resource.items.id
  http_method = aws_api_gateway_method.get_items.http_method
  type        = "AWS_PROXY"
  integration_http_method = "POST"
  uri = aws_lambda_function.handler.invoke_arn
}
```

**HTTP API (v2):**
```hcl
resource "aws_apigatewayv2_api" "api" {
  name          = "my-api"
  protocol_type = "HTTP"
}
resource "aws_apigatewayv2_integration" "lambda" {
  api_id           = aws_apigatewayv2_api.api.id
  integration_type = "AWS_PROXY"
  integration_uri  = aws_lambda_function.handler.invoke_arn
}
resource "aws_apigatewayv2_route" "get" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "GET /items"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}
```

### Multiple methods on same path only hit one function

Each HTTP method needs its own `aws_api_gateway_method` and `aws_api_gateway_integration` pair. A common mistake is creating the method but forgetting the integration.

### CORS preflight fails

Lambdaform includes built-in CORS support. Configure it in `lambdaform.yaml`:

```yaml
cors:
  allow_origins:
    - "http://localhost:3000"
    - "http://localhost:5173"
  allow_methods:
    - GET
    - POST
    - PUT
    - DELETE
    - OPTIONS
  allow_headers:
    - Content-Type
    - Authorization
```

Or allow everything for development:

```yaml
cors:
  allow_origins: ["*"]
```

## Runtime Issues

### Handler function not found

The `handler` attribute format is `filename.functionName`:

- **Node.js:** `index.handler` → looks for `exports.handler` in `index.js`
- **Python:** `handler.handle` → looks for `def handle()` in `handler.py`
- **Nested paths:** `src/api.handler` → looks in `src/api.js` or `src/api.py`

Lambdaform searches for the handler file in:
1. The `source_path` directory (if configured)
2. The project root
3. Common subdirectories (`src/`, `lambda/`, `functions/`)

### console.log / print output missing

Lambdaform redirects stdout from Lambda functions to preserve the worker protocol. Your `console.log()` (Node.js) and `print()` (Python) output appears in stderr, which Lambdaform displays as log lines prefixed with the function name:

```
[hello-world] Processing request for /items
```

### Python import errors

If your Lambda function imports third-party packages, ensure they're installed in the source directory:

```bash
cd my-lambda-source/
pip install -r requirements.txt -t .
```

Or use Lambda layers for shared dependencies.

## Terraform Parsing Issues

### Variables show as `${var.name}` instead of values

Create a `terraform.tfvars` or `*.auto.tfvars` file with your variable values:

```hcl
# terraform.tfvars
environment = "dev"
table_name  = "my-table"
```

Or pass them via CLI:

```bash
lambdaform start --var-file=dev.tfvars
```

### `count` or `for_each` resources not found

Lambdaform parses `.tf` files statically — it doesn't evaluate `count` or `for_each` expressions. Resources using these will generate a warning. As a workaround, create a simplified `.tf` file for local development without dynamic resource counts.

### Nested Terraform modules

Lambdaform supports local modules up to 3+ levels deep. Ensure your modules use relative paths:

```hcl
module "api" {
  source = "./modules/api"
}
```

Remote modules (from registries) are not supported — Lambdaform only reads local `.tf` files.

### `jsonencode()` not resolving

Lambdaform supports `jsonencode()` and common Terraform functions (`lookup`, `concat`, `merge`, `coalesce`, `lower`, `upper`, `replace`, `join`, `split`, `trimprefix`, `trimsuffix`, `format`). Complex expressions inside these functions may not fully resolve — the tool will use the raw expression as a fallback.

## WebSocket Issues

### @connections API returns connection refused

The @connections management API runs on the WebSocket port + 1. If your WebSocket is on port 3001, the @connections API is at `http://localhost:3002`.

```bash
# Send message to a connection
curl -X POST http://localhost:3002/@connections/{connectionId} \
  -d '{"message": "hello"}'
```

### $default route not receiving messages

Ensure your route selection expression matches your message format. The default is `$request.body.action`, meaning messages should be JSON with an `action` field:

```json
{"action": "sendMessage", "data": "hello"}
```

Messages where the `action` field doesn't match any route (or non-JSON messages) fall through to `$default`.

## Performance

### Slow first request

The first request to each function spawns a new worker process (cold start). Subsequent requests reuse the warm worker (~3ms). This mirrors AWS Lambda's cold/warm start behavior.

### High memory usage with many functions

Each function gets its own worker process. If you have many functions, consider using `--function` to start only the ones you need:

```bash
lambdaform start --function my-api-handler
```

## Still stuck?

1. Run with verbose logging: `RUST_LOG=debug lambdaform start`
2. Validate your config: `lambdaform validate`
3. Check the [GitHub Discussions](https://github.com/ConnerV42/lambdaform/discussions)
4. File an issue: [GitHub Issues](https://github.com/ConnerV42/lambdaform/issues)
