# REST API Gateway (v1)
resource "aws_api_gateway_rest_api" "rest_api" {
  name = "rest-api"
}

resource "aws_api_gateway_resource" "users" {
  rest_api_id = aws_api_gateway_rest_api.rest_api.id
  parent_id   = aws_api_gateway_rest_api.rest_api.root_resource_id
  path_part   = "users"
}

resource "aws_api_gateway_method" "get_users" {
  rest_api_id   = aws_api_gateway_rest_api.rest_api.id
  resource_id   = aws_api_gateway_resource.users.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "get_users" {
  rest_api_id             = aws_api_gateway_rest_api.rest_api.id
  resource_id             = aws_api_gateway_resource.users.id
  http_method             = aws_api_gateway_method.get_users.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.list_users.invoke_arn
}

# HTTP API Gateway (v2)
resource "aws_apigatewayv2_api" "http_api" {
  name          = "http-api"
  protocol_type = "HTTP"
}

resource "aws_apigatewayv2_route" "get_items" {
  api_id    = aws_apigatewayv2_api.http_api.id
  route_key = "GET /items"
  target    = aws_apigatewayv2_integration.get_items.id
}

resource "aws_apigatewayv2_integration" "get_items" {
  api_id             = aws_apigatewayv2_api.http_api.id
  integration_type   = "AWS_PROXY"
  integration_uri    = aws_lambda_function.list_items.invoke_arn
}

# Lambda functions
resource "aws_lambda_function" "list_users" {
  function_name = "list-users"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "users.zip"
}

resource "aws_lambda_function" "list_items" {
  function_name = "list-items"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "items.zip"
}
