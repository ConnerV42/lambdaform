# API Gateway Routing

Lambdaform supports both API Gateway versions. It parses your Terraform resources and builds a local route table automatically.

## REST API (v1)

Terraform resources used:
- `aws_api_gateway_rest_api`
- `aws_api_gateway_resource` (path segments)
- `aws_api_gateway_method` (HTTP method binding)
- `aws_api_gateway_integration` (Lambda proxy)

```hcl
resource "aws_api_gateway_rest_api" "api" {
  name = "my-api"
}

resource "aws_api_gateway_resource" "users" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "users"
}

resource "aws_api_gateway_resource" "user" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_resource.users.id
  path_part   = "{userId}"
}

resource "aws_api_gateway_method" "get_user" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.user.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "get_user" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  resource_id = aws_api_gateway_resource.user.id
  http_method = "GET"
  type        = "AWS_PROXY"
  uri         = aws_lambda_function.get_user.invoke_arn
}
```

This creates the route `GET /users/{userId}` → `get_user` function.

### Path Parameters

Path parameters use `{param}` syntax in `path_part`. They're passed to your Lambda in `event.pathParameters`:

```javascript
exports.handler = async (event) => {
  const userId = event.pathParameters.userId;
  // ...
};
```

### Nested Resources

Lambdaform resolves nested `aws_api_gateway_resource` chains by following `parent_id` references. Deeply nested paths like `/api/v1/users/{id}/orders` work correctly.

## HTTP API (v2)

Terraform resources used:
- `aws_apigatewayv2_api`
- `aws_apigatewayv2_route`
- `aws_apigatewayv2_integration`

```hcl
resource "aws_apigatewayv2_api" "http_api" {
  name          = "http-api"
  protocol_type = "HTTP"
}

resource "aws_apigatewayv2_integration" "handler" {
  api_id             = aws_apigatewayv2_api.http_api.id
  integration_type   = "AWS_PROXY"
  integration_uri    = aws_lambda_function.handler.invoke_arn
  payload_format_version = "2.0"
}

resource "aws_apigatewayv2_route" "get_items" {
  api_id    = aws_apigatewayv2_api.http_api.id
  route_key = "GET /items"
  target    = "integrations/${aws_apigatewayv2_integration.handler.id}"
}
```

### Event Format

HTTP API v2 uses a different event format than REST API v1. Lambdaform sends the correct format based on the gateway type:

- **v1 (REST):** `event.httpMethod`, `event.pathParameters`, `event.queryStringParameters`
- **v2 (HTTP):** `event.requestContext.http.method`, `event.rawPath`, `event.rawQueryString`

## Viewing Routes

Use `lambdaform config` to see all discovered routes:

```bash
lambdaform config
```

```
📦 Lambda Functions:
   • get-user (Nodejs20) → users.getHandler
   • create-user (Nodejs20) → users.createHandler

🌐 API Gateway: my-api (REST)
   GET    /users/{userId}  → get-user
   POST   /users           → create-user
```
