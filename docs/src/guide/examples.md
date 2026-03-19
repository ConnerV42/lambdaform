# Examples & Cookbook

Lambdaform ships with 18 complete example projects, each demonstrating a different pattern.
Browse them in the [`examples/`](https://github.com/ConnerV42/lambdaform/tree/main/examples) directory, or use this page as a quick reference for common patterns.

## Quick Start

Every example follows the same workflow:

```bash
cd examples/<project-name>
lambdaform start
# In another terminal:
curl http://localhost:3000/...
```

---

## REST API (CRUD)

**Example:** `crud-api-node` / `crud-api-python`

The most common Lambda pattern — a REST API with GET, POST, PUT, DELETE.

```hcl
resource "aws_api_gateway_rest_api" "api" {
  name = "my-api"
}

resource "aws_lambda_function" "handler" {
  function_name = "crud-handler"
  runtime       = "nodejs20.x"
  handler       = "index.handler"
  filename      = "lambda.zip"
}
```

```bash
lambdaform start

# List items
curl http://localhost:3000/items

# Create item
curl -X POST http://localhost:3000/items \
  -H "Content-Type: application/json" \
  -d '{"name": "Widget", "description": "A fine widget"}'

# Get by ID
curl http://localhost:3000/items/1

# Update
curl -X PUT http://localhost:3000/items/1 \
  -H "Content-Type: application/json" \
  -d '{"name": "Updated Widget"}'

# Delete
curl -X DELETE http://localhost:3000/items/1
```

Both Node.js and Python versions produce identical API behavior, demonstrating runtime parity.

---

## HTTP API (v2 Event Format)

**Example:** `api-gateway-v2`

HTTP APIs use a simpler, faster event format with `version: "2.0"`.

```hcl
resource "aws_apigatewayv2_api" "http_api" {
  name          = "http-api"
  protocol_type = "HTTP"
}
```

Key differences from REST API:
- Simpler event structure (`rawPath`, `rawQueryString` instead of nested objects)
- Cookie extraction built-in
- `$default` catch-all route support
- Lower latency in production

```bash
lambdaform start
curl http://localhost:3000/hello
curl http://localhost:3000/echo -X POST -d '{"msg": "hi"}'
```

---

## Multiple Functions & Layers

**Example:** `multi-function`

Split your API across multiple Lambda functions with shared code via layers.

```hcl
resource "aws_lambda_layer_version" "shared" {
  layer_name          = "shared-utils"
  compatible_runtimes = ["nodejs20.x"]
  filename            = "layer.zip"
  source_code_hash    = filebase64sha256("layer.zip")
}

resource "aws_lambda_function" "users" {
  function_name = "users-handler"
  runtime       = "nodejs20.x"
  handler       = "users.handler"
  layers        = [aws_lambda_layer_version.shared.arn]
}

resource "aws_lambda_function" "products" {
  function_name = "products-handler"
  runtime       = "python3.12"
  handler       = "products.handler"
  layers        = [aws_lambda_layer_version.shared.arn]
}
```

Lambdaform prepends layer source paths to `NODE_PATH` / `PYTHONPATH` automatically, just like AWS does.

---

## WebSocket API

**Example:** `websocket-chat`

Build real-time applications with WebSocket API Gateway.

```hcl
resource "aws_apigatewayv2_api" "ws" {
  name                       = "chat-ws"
  protocol_type              = "WEBSOCKET"
  route_selection_expression = "$request.body.action"
}
```

Three required routes:
- `$connect` — client connects
- `$disconnect` — client disconnects
- `$default` — catch-all for messages

```bash
lambdaform start

# Connect with wscat
npx wscat -c ws://localhost:3001
> {"action": "sendMessage", "message": "Hello!"}
```

The `@connections` management API runs on a separate port (shown at startup) for posting messages back to connected clients.

---

## SQS & SNS Triggers

**Examples:** `sqs-processor`, `sns-fanout`

Test event-driven architectures locally with `lambdaform trigger`.

```bash
# SQS trigger
lambdaform trigger -t sqs -s my-queue -m '{"orderId": "123"}'

# SQS batch (5 messages)
lambdaform trigger -t sqs -s my-queue -m '{"orderId": "123"}' -b 5

# SNS trigger
lambdaform trigger -t sns -s my-topic -m '{"alert": "high CPU"}'

# Dry run (see event without invoking)
lambdaform trigger -t sqs -s my-queue -m '{"test": true}' --dry-run
```

Lambdaform reads `aws_lambda_event_source_mapping` and `aws_sns_topic_subscription` resources from your Terraform to automatically route triggers to the correct function.

---

## Step Functions

**Example:** `step-functions`

Visualize and test state machines locally.

```bash
# Visualize the state machine
lambdaform sfn

# Output as JSON
lambdaform sfn --json

# Invoke a specific state's Lambda
lambdaform invoke validate-order -e '{"orderId": "abc"}'
```

Supports Task, Choice, Parallel, Wait, Pass, Succeed, and Fail states.

---

## Lambda Authorizers

**Example:** `authorizer-flow`

Test protected APIs with TOKEN or REQUEST authorizers.

```bash
lambdaform start

# No token → 401
curl http://localhost:3000/protected

# Valid token → 200
curl http://localhost:3000/protected \
  -H "Authorization: Bearer valid-token"

# Public route (no auth required)
curl http://localhost:3000/public
```

---

## Go & Rust Runtimes

**Examples:** `go-lambda`, `rust-lambda`

Custom runtimes using `provided.al2023`.

```bash
# Go
cd examples/go-lambda
lambdaform start   # auto-detects Go, runs `go build`

# Rust
cd examples/rust-lambda
lambdaform start   # auto-detects Cargo.toml, runs `cargo build`
```

Lambdaform auto-detects the build tool, compiles for your platform, and runs the binary directly. Hot reload triggers rebuilds on source changes.

---

## Terraform Modules

**Example:** `terraform-modules`

Lambdaform discovers Lambda functions inside nested modules up to any depth.

```
project/
├── main.tf              # Root module
├── modules/
│   ├── api/
│   │   ├── main.tf      # Declares Lambda + API Gateway
│   │   └── modules/
│   │       └── auth/
│   │           └── main.tf  # Declares authorizer Lambda
│   └── workers/
│       └── main.tf      # Declares SQS-triggered Lambda
```

```bash
lambdaform config   # Shows all discovered functions with module prefixes
lambdaform start    # All functions available on their respective ports
```

---

## Multiple API Gateways (Monorepo)

**Example:** `monorepo`

Run multiple API Gateways simultaneously on different ports.

```bash
lambdaform start
# Gateway 1: http://localhost:3000  (public API)
# Gateway 2: http://localhost:3001  (admin API)
# Gateway 3: http://localhost:3002  (internal API)
```

Each gateway gets its own port, and routes are correctly assigned to their respective gateway.

---

## Lambda Function URLs

**Example:** `function-urls`

Direct HTTPS endpoints without API Gateway.

```hcl
resource "aws_lambda_function_url" "greeting" {
  function_name      = aws_lambda_function.greeting.function_name
  authorization_type = "NONE"

  cors {
    allow_origins = ["*"]
    allow_methods = ["GET", "POST"]
  }
}
```

Each function URL gets its own port. CORS is configured per-function.

---

## Docker Compose Integration

**Examples:** `docker-compose-dynamodb`, `docker-compose-fullstack`

Combine Lambdaform with DynamoDB Local, LocalStack, or other services.

```yaml
# docker-compose.yml
services:
  dynamodb-local:
    image: amazon/dynamodb-local
    ports:
      - "8000:8000"

  lambdaform:
    build: .
    ports:
      - "3000:3000"
    environment:
      - DYNAMODB_ENDPOINT=http://dynamodb-local:8000
    depends_on:
      - dynamodb-local
```

```bash
docker compose up
curl http://localhost:3000/items
```

---

## Infrastructure Graph

Visualize your Lambda architecture:

```bash
# ASCII art
lambdaform graph

# Graphviz DOT format
lambdaform graph --format dot | dot -Tpng -o graph.png

# JSON for programmatic use
lambdaform graph --format json

# Show port assignments
lambdaform graph --port 3000
```

---

## Cost Estimation

Estimate AWS costs from local usage:

```bash
# After running some requests
lambdaform cost

# JSON output
lambdaform cost --json
```

Shows per-function breakdown with duration, memory, and monthly projection (including free tier).

---

## Debugging

### Node.js
```bash
lambdaform start --debug
# Attach Chrome DevTools to chrome://inspect or VS Code debugger to port 9229
```

### Python
```bash
lambdaform start --debug-python
# Attach VS Code debugger to port 5678
```

---

## Tips

- **Hot reload** is on by default — edit your Lambda code and Lambdaform rebuilds automatically
- **Environment variables** from your Terraform `environment` blocks are passed to functions
- **`lambdaform validate`** catches Terraform parsing errors before you start
- **`lambdaform config --json`** dumps the full parsed config for debugging
- **Process pooling** keeps Node.js/Python workers warm between requests for faster response times
- **`--verbose`** shows full request/response headers and bodies
- **`--json-log`** outputs structured JSON for log aggregation tools
