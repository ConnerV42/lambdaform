# HTTP API Gateway (v2) test fixture for Lambdaform

resource "aws_lambda_function" "hello" {
  function_name = "hello-http"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  timeout       = 30
  memory_size   = 128

  filename = "lambda.zip"

  environment {
    variables = {
      GREETING = "Hello from HTTP API!"
      ENV      = "local"
    }
  }
}

resource "aws_lambda_function" "users" {
  function_name = "users-http"
  handler       = "users.handler"
  runtime       = "nodejs20.x"
  timeout       = 10
  memory_size   = 128

  filename = "lambda.zip"
}

# HTTP API (v2)
resource "aws_apigatewayv2_api" "api" {
  name          = "hello-http-api"
  protocol_type = "HTTP"
}

# Integration: connects API to Lambda
resource "aws_apigatewayv2_integration" "hello" {
  api_id             = aws_apigatewayv2_api.api.id
  integration_type   = "AWS_PROXY"
  integration_uri    = aws_lambda_function.hello.invoke_arn
  payload_format_version = "2.0"
}

resource "aws_apigatewayv2_integration" "users" {
  api_id             = aws_apigatewayv2_api.api.id
  integration_type   = "AWS_PROXY"
  integration_uri    = aws_lambda_function.users.invoke_arn
  payload_format_version = "2.0"
}

# Routes
resource "aws_apigatewayv2_route" "get_hello" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "GET /hello"
  target    = aws_apigatewayv2_integration.hello.id
}

resource "aws_apigatewayv2_route" "post_hello" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "POST /hello"
  target    = aws_apigatewayv2_integration.hello.id
}

resource "aws_apigatewayv2_route" "get_users" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "GET /users/{id}"
  target    = aws_apigatewayv2_integration.users.id
}

resource "aws_apigatewayv2_route" "default" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "$default"
  target    = aws_apigatewayv2_integration.hello.id
}
