# 🚀 Lambdaform

> The only local Lambda tool that reads your Terraform

**Lambdaform** is a Terraform-native local development server for AWS Lambda + API Gateway. No CloudFormation. No Docker. No LocalStack account. Just your Terraform files and your code.

## Why?

If you use Terraform for Lambda infrastructure, your local development options are painful:

| Tool | Terraform-Native | Free | No Docker | Hot Reload |
|------|-----------------|------|-----------|------------|
| LocalStack | ❌ | ⚠️ Limited | ❌ | ❌ (Pro) |
| SAM CLI | ⚠️ Beta | ✅ | ❌ | ❌ |
| serverless-offline | ❌ | ✅ | ⚠️ | ✅ |
| **Lambdaform** | ✅ | ✅ | ✅ | ✅ |

## Features

- 📖 **Reads your Terraform** — no separate configuration file
- ⚡ **Fast startup** — single binary, no Docker
- 🔥 **Hot reload** — instant feedback on code changes
- 🆓 **Open source** — MIT license, no feature gating

## Quick Start

```bash
# Install
brew install lambdaform  # or cargo install lambdaform

# Start in your Terraform project
cd my-terraform-project
lambdaform start

# That's it! Server runs at http://localhost:3000
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

Lambdaform automatically creates routes:

```
$ lambdaform start

┌─────────────────────────────────────────┐
│           🚀 Lambdaform v0.1.0          │
│     Terraform-native Lambda emulator    │
└─────────────────────────────────────────┘

📦 Loaded 1 Lambda function:
   • api_handler (nodejs20.x) → index.handler

🌐 API Gateway routes:
   GET /users → api_handler

🔥 Server running at http://localhost:3000
👀 Watching for changes...
```

## CLI Usage

```bash
# Start server
lambdaform start
lambdaform start --port 8080
lambdaform start --dir ./infra

# Invoke function directly
lambdaform invoke my_function --event '{"key": "value"}'
lambdaform invoke my_function --event-file event.json

# Show parsed configuration
lambdaform config
lambdaform config --json

# Validate Terraform files
lambdaform validate
```

## Supported Runtimes

| Runtime | Status |
|---------|--------|
| Node.js 18.x | ✅ Supported |
| Node.js 20.x | ✅ Supported |
| Python 3.10 | ✅ Supported |
| Python 3.11 | ✅ Supported |
| Python 3.12 | ✅ Supported |
| Go 1.x | 🚧 Planned |
| Rust (provided.al2023) | 🚧 Planned |

## Configuration (Optional)

For cases where HCL parsing needs hints, create `lambdaform.yaml`:

```yaml
version: 1

functions:
  api_handler:
    source: ./dist  # Override source path

environment:
  TABLE_NAME: local-table  # Local-only env vars

gateway:
  port: 3000
  cors: true
```

## Roadmap

- [x] Parse `aws_lambda_function` from HCL
- [x] Node.js runtime
- [x] Python runtime
- [x] API Gateway REST (v1) routing
- [x] Hot reload
- [ ] API Gateway HTTP (v2)
- [ ] Lambda authorizers
- [ ] WebSocket support
- [ ] VS Code extension
- [ ] Debugger integration

## Contributing

PRs welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT
