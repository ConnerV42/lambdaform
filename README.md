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

## Demo

```bash
$ lambdaform validate
🔍 Validating Terraform in: ./
   Found 2 .tf file(s)
   Found 3 function(s), 1 gateway(s), 3 route(s)
✅ Validation passed!

$ lambdaform config
📂 Parsed from: ./

📦 Lambda Functions (3):
   • hello-world (Nodejs20)
     Handler: index.handler
     Timeout: 30s, Memory: 128MB
     Env vars: ["ENV", "GREETING"]
   • echo (Nodejs20)
     Handler: echo.handler
     Timeout: 10s, Memory: 128MB
   • get-user (Nodejs20)
     Handler: users.handler
     Timeout: 10s, Memory: 128MB

🌐 API Gateways (1):
   • hello-api (Rest)
     Get /hello → hello
     Post /echo → echo
     Get /users/{id} → get_user
```

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

Lambdaform parses it and creates routes automatically:

```
$ lambdaform config

📂 Parsed from: ./

📦 Lambda Functions (1):
   • my-api (Nodejs20)
     Handler: index.handler
     Timeout: 30s, Memory: 128MB
     Env vars: ["TABLE_NAME"]

🌐 API Gateways (1):
   • my-api (Rest)
     Get /users → api_handler

$ lambdaform start

🚀 Lambdaform v0.1.0
📦 Loaded 1 function, 1 route
🔥 Server running at http://localhost:3000
👀 Watching for changes...

$ curl http://localhost:3000/users
{"statusCode":200,"body":"[{\"id\":1,\"name\":\"Alice\"}]"}
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
