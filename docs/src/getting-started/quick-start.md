# Quick Start

This guide walks you through running Lambdaform with a simple Lambda + API Gateway project in under 5 minutes.

## 1. Create a project

```bash
mkdir my-lambda-app && cd my-lambda-app
```

## 2. Write your Terraform

Create `main.tf`:

```hcl
resource "aws_lambda_function" "hello" {
  function_name = "hello"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "hello.zip"

  environment {
    variables = {
      GREETING = "Hello from Lambdaform!"
    }
  }
}

resource "aws_api_gateway_rest_api" "api" {
  name = "my-api"
}

resource "aws_api_gateway_resource" "hello" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "hello"
}

resource "aws_api_gateway_method" "get_hello" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.hello.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "get_hello" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  resource_id = aws_api_gateway_resource.hello.id
  http_method = aws_api_gateway_method.get_hello.http_method
  type        = "AWS_PROXY"
  uri         = aws_lambda_function.hello.invoke_arn
}
```

## 3. Write your handler

Create `index.js`:

```javascript
exports.handler = async (event) => {
  const name = event.queryStringParameters?.name || "World";
  return {
    statusCode: 200,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      message: `${process.env.GREETING} Welcome, ${name}!`,
    }),
  };
};
```

## 4. Start Lambdaform

```bash
lambdaform start
```

Output:
```
🚀 Lambdaform dev server
📦 Loaded 1 function, 1 route
🔥 Hot reload enabled — watching for changes
🌐 http://localhost:3000
```

## 5. Test it

```bash
curl http://localhost:3000/hello?name=Developer
```

```json
{
  "message": "Hello from Lambdaform! Welcome, Developer!"
}
```

## 6. Edit and reload

Change your handler code — Lambdaform detects the change and reloads automatically. No restart needed.

## What's Next?

- [Project Setup](./project-setup.md) — use `lambdaform init` for guided setup
- [CLI Reference](../guide/cli-reference.md) — all commands and flags
- [Configuration](../guide/configuration.md) — customize with `lambdaform.yaml`
- [API Gateway Routing](../guide/api-gateway.md) — REST vs HTTP API details
