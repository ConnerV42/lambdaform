# Two separate REST API Gateways — routes must be assigned to correct gateway

resource "aws_api_gateway_rest_api" "users_api" {
  name = "users-api"
}

resource "aws_api_gateway_rest_api" "orders_api" {
  name = "orders-api"
}

# Users API resources
resource "aws_api_gateway_resource" "users" {
  rest_api_id = aws_api_gateway_rest_api.users_api.id
  parent_id   = aws_api_gateway_rest_api.users_api.root_resource_id
  path_part   = "users"
}

resource "aws_api_gateway_method" "get_users" {
  rest_api_id   = aws_api_gateway_rest_api.users_api.id
  resource_id   = aws_api_gateway_resource.users.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "get_users" {
  rest_api_id             = aws_api_gateway_rest_api.users_api.id
  resource_id             = aws_api_gateway_resource.users.id
  http_method             = aws_api_gateway_method.get_users.http_method
  type                    = "AWS_PROXY"
  integration_http_method = "POST"
  uri                     = aws_lambda_function.list_users.invoke_arn
}

# Orders API resources
resource "aws_api_gateway_resource" "orders" {
  rest_api_id = aws_api_gateway_rest_api.orders_api.id
  parent_id   = aws_api_gateway_rest_api.orders_api.root_resource_id
  path_part   = "orders"
}

resource "aws_api_gateway_method" "create_order" {
  rest_api_id   = aws_api_gateway_rest_api.orders_api.id
  resource_id   = aws_api_gateway_resource.orders.id
  http_method   = "POST"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "create_order" {
  rest_api_id             = aws_api_gateway_rest_api.orders_api.id
  resource_id             = aws_api_gateway_resource.orders.id
  http_method             = aws_api_gateway_method.create_order.http_method
  type                    = "AWS_PROXY"
  integration_http_method = "POST"
  uri                     = aws_lambda_function.create_order.invoke_arn
}

# Lambda functions
resource "aws_lambda_function" "list_users" {
  function_name = "list-users"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "users.zip"
}

resource "aws_lambda_function" "create_order" {
  function_name = "create-order"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "orders.zip"
}
