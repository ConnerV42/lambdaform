# 🚀 Lambdaform

[![Documentation](https://img.shields.io/badge/docs-lambdaform-blue)](https://connerv42.github.io/lambdaform/)

> The only local Lambda tool that reads your Terraform

**Lambdaform** is a Terraform-native local development server for AWS Lambda + API Gateway. No CloudFormation. No LocalStack account. Just your Terraform files and your code. Most runtimes run natively — no Docker required. Java runtimes use Docker for JVM environment parity.

## Why?

If you use Terraform for Lambda infrastructure, your local development options are painful:

| Tool | Terraform-Native | Free | No Docker | Hot Reload |
|------|-----------------|------|-----------|------------|
| LocalStack | ❌ | ⚠️ Limited | ❌ | ❌ (Pro) |
| SAM CLI | ⚠️ Beta | ✅ | ❌ | ❌ |
| serverless-offline | ❌ | ✅ | ⚠️ | ✅ |
| **Lambdaform** | ✅ | ✅ | ✅¹ | ✅ |

¹ *Node.js, Python, Go, and Rust run natively. Java runtimes use Docker for JVM parity.*

## Features

- 📖 **Reads your Terraform** — no separate configuration file needed
- ⚡ **Fast startup** — single binary, no Docker required for most runtimes
- 🔥 **Hot reload** — instant feedback on code and `.tf` changes
- 🌐 **REST & HTTP APIs** — supports both API Gateway v1 (REST) and v2 (HTTP)
- 🔒 **Lambda authorizers** — TOKEN and REQUEST authorizer support
- 🐛 **Debugger integration** — attach Node.js (`--inspect-brk`) and Python (`debugpy`) debuggers
- 🏎️ **Warm process pool** — ~3ms warm invocations (97% faster than cold start)
- 🔀 **Multiple gateways** — each API gets its own port
- 🌍 **CORS** — built-in, configurable via `lambdaform.yaml`
- 📦 **Lambda layers** — automatic layer path resolution for Node.js and Python
- 🔌 **WebSocket APIs** — `$connect`/`$disconnect`/`$default`/custom routes with @connections management
- 📨 **SQS/SNS triggers** — simulate event source mappings with proper event payloads
- 🗄️ **DynamoDB hints** — table parsing with DynamoDB Local setup guidance
- 📊 **Step Functions viz** — ASCII flow diagrams from `aws_sfn_state_machine` definitions
- 🔄 **OpenTofu compatible** — works identically with OpenTofu and Terraform `.tf` files
- 🆓 **Open source** — MIT license, no feature gating

## Quick Start

```bash
# Install via Homebrew (macOS/Linux)
brew tap ConnerV42/lambdaform
brew install lambdaform

# Or via Cargo
cargo install lambdaform

# Start in your Terraform project
cd my-terraform-project
lambdaform start

# That's it! Server runs at http://localhost:3000
```

## Demo

[![asciicast](https://asciinema.org/a/N79Qw2QUHtMwJL9F.svg)](https://asciinema.org/a/N79Qw2QUHtMwJL9F)

<details>
<summary>Text version</summary>

```bash
$ lambdaform validate
🔍 Validating Terraform in: ./
   Found 1 .tf file(s)
   Found 3 function(s), 1 gateway(s), 3 route(s)
✅ Validation passed!

$ lambdaform start
📦 Lambda Functions:
   • hello-world (Nodejs20) → index.handler
   • echo (Nodejs20) → echo.handler
   • get-user (Nodejs20) → users.handler
🔥 Server running at http://localhost:3000

$ curl -s localhost:3000/hello?name=World | jq .
{
  "message": "Hello from Lambdaform! Welcome, World!",
  "environment": "local",
  "requestId": "local-a1b2c3d4"
}
```
</details>

## Supported Runtimes

| Runtime | Status | Invocation |
|---------|--------|------------|
| Node.js 18.x / 20.x | ✅ | Warm pool (~3ms) |
| Python 3.10 / 3.11 / 3.12 | ✅ | Warm pool (~3ms) |
| Go 1.x / provided.al2 / provided.al2023 | ✅ | Mini RIE (~14ms) |
| Rust (provided.al2023) | ✅ | Mini RIE (~14ms) |
| Java 8/11/17/21 | ✅ | Docker (~500ms) |

## CLI Usage

```bash
# Start server
lambdaform start
lambdaform start --port 8080
lambdaform start --dir ./infra
lambdaform start --verbose          # detailed request logging

# Debugger mode
lambdaform start --debug                    # Node.js (port 9229)
lambdaform start --debug-python             # Python debugpy (port 5678)
lambdaform start --debug-port 9230          # custom port

# Invoke function directly
lambdaform invoke my_function --event '{"key": "value"}'
lambdaform invoke my_function --event-file event.json

# Show parsed configuration
lambdaform config
lambdaform config --json

# Validate Terraform files
lambdaform validate

# Step Functions visualization
lambdaform stepfunctions                   # ASCII flow diagrams
lambdaform sfn                             # alias

# Trigger SQS/SNS events
lambdaform trigger sqs my_queue '{"key":"value"}'
lambdaform trigger sns my_topic '{"key":"value"}'
lambdaform trigger sqs my_queue '{"key":"value"}' --batch 5
lambdaform trigger sqs my_queue '{"key":"value"}' --function my_handler
```

## Example

Given this Terraform:

```hcl
resource "aws_lambda_function" "api_handler" {
  function_name = "my-api"
  handler       = "index.handler"
  runtime       = "nodejs20.x"

  environment {
    variables = {
      TABLE_NAME = "users"
    }
  }
}

resource "aws_api_gateway_rest_api" "api" {
  name = "my-api"
}

resource "aws_api_gateway_resource" "users" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "users"
}

resource "aws_api_gateway_method" "get_users" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.users.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "get_users" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  resource_id = aws_api_gateway_resource.users.id
  http_method = aws_api_gateway_method.get_users.http_method
  type        = "AWS_PROXY"
  uri         = aws_lambda_function.api_handler.invoke_arn
}
```

Lambdaform parses it and starts a local server automatically:

```
$ lambdaform start
🚀 Lambdaform dev server
📦 Loaded 1 function, 1 route
🔥 Hot reload enabled — watching for changes
🌐 http://localhost:3000

$ curl http://localhost:3000/users
{"statusCode":200,"body":"[{\"id\":1,\"name\":\"Alice\"}]"}
```

## Configuration (Optional)

For cases where HCL parsing needs hints, or to customize local behavior, create `lambdaform.yaml`:

```yaml
version: 1

# Override function settings
functions:
  api_handler:
    source: ./dist
    handler: build/index.handler
    timeout: 30
    memory: 256
    environment:
      TABLE_NAME: local-table

# Global environment variables (applied to all functions)
environment:
  AWS_REGION: us-west-2
  STAGE: local

# Gateway settings
gateway:
  port: 3000

# Per-gateway port overrides (for multi-gateway projects)
gateways:
  public_api:
    port: 3000
  admin_api:
    port: 3001

# CORS configuration
cors:
  allow_origins:
    - "http://localhost:5173"
  allow_methods:
    - GET
    - POST
  allow_headers:
    - Content-Type
    - Authorization

# Watch settings
watch:
  enabled: true
  ignore:
    - node_modules
    - .terraform

# Debugger settings
debug:
  enabled: false
  port: 9229          # Node.js inspector port
  python: false
  python_port: 5678   # debugpy port
```

## Lambda Authorizers

Lambdaform supports v1 TOKEN/REQUEST and v2 REQUEST authorizers. Define them in your Terraform as usual — Lambdaform executes the authorizer Lambda before the target handler:

```hcl
resource "aws_api_gateway_authorizer" "token_auth" {
  name            = "token-auth"
  rest_api_id     = aws_api_gateway_rest_api.api.id
  type            = "TOKEN"
  authorizer_uri  = aws_lambda_function.authorizer.invoke_arn
}
```

The authorizer runs first. If it returns a deny policy or throws, Lambdaform returns `401 Unauthorized`.

## Multiple API Gateways

Projects with multiple gateways get separate ports automatically:

```
🌐 public-api → http://localhost:3000
🌐 admin-api  → http://localhost:3001
```

Override ports in `lambdaform.yaml` under the `gateways` section.

## Debugger Integration

### Node.js
```bash
lambdaform start --debug
# Attach VS Code or Chrome DevTools to localhost:9229
```

### Python
```bash
lambdaform start --debug-python
# Attach VS Code (Python: Remote Attach) to localhost:5678
```

Debug mode disables the warm process pool so breakpoints work correctly.

## Roadmap

- [x] Parse `aws_lambda_function` from HCL
- [x] Node.js runtime (18.x, 20.x)
- [x] Python runtime (3.10–3.12)
- [x] Go runtime (go1.x, provided.al2/al2023)
- [x] API Gateway REST (v1) routing
- [x] API Gateway HTTP (v2) routing
- [x] Lambda authorizers (TOKEN + REQUEST)
- [x] Hot reload
- [x] CORS handling
- [x] Warm process pooling
- [x] Debugger integration (Node.js + Python)
- [x] Multiple API Gateway support
- [x] Config file (`lambdaform.yaml`)
- [x] Lambda layers
- [x] WebSocket API Gateway support
- [x] SQS/SNS trigger simulation
- [x] DynamoDB Local integration hints
- [x] Step Functions visualization (read-only)

## Contributing

PRs welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT
