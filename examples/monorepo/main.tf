# Monorepo example — Multiple API Gateways on different ports
# Tests: multi-gateway routing, mixed runtimes, shared layers

variable "environment" {
  default = "dev"
}

locals {
  prefix = "monorepo-${var.environment}"
}

# Shared utilities layer
resource "aws_lambda_layer_version" "shared_utils" {
  layer_name          = "${local.prefix}-shared-utils"
  filename            = "shared/utils.js"
  compatible_runtimes = ["nodejs20.x", "python3.12"]
}

# ===== Users API (REST v1) =====
resource "aws_api_gateway_rest_api" "users_api" {
  name = "${local.prefix}-users-api"
}

resource "aws_lambda_function" "list_users" {
  function_name = "${local.prefix}-list-users"
  handler       = "handler.list_users"
  runtime       = "nodejs20.x"
  filename      = "services/users/handler.js"
  role          = "arn:aws:iam::123456789012:role/lambda-role"
  timeout       = 10
  layers        = [aws_lambda_layer_version.shared_utils.arn]

  environment {
    variables = {
      TABLE_NAME = "users-${var.environment}"
    }
  }
}

resource "aws_lambda_function" "get_user" {
  function_name = "${local.prefix}-get-user"
  handler       = "handler.get_user"
  runtime       = "nodejs20.x"
  filename      = "services/users/handler.js"
  role          = "arn:aws:iam::123456789012:role/lambda-role"
  timeout       = 10

  environment {
    variables = {
      TABLE_NAME = "users-${var.environment}"
    }
  }
}

resource "aws_api_gateway_resource" "users" {
  rest_api_id = aws_api_gateway_rest_api.users_api.id
  parent_id   = aws_api_gateway_rest_api.users_api.root_resource_id
  path_part   = "users"
}

resource "aws_api_gateway_resource" "user_by_id" {
  rest_api_id = aws_api_gateway_rest_api.users_api.id
  parent_id   = aws_api_gateway_resource.users.id
  path_part   = "{userId}"
}

resource "aws_api_gateway_method" "list_users" {
  rest_api_id   = aws_api_gateway_rest_api.users_api.id
  resource_id   = aws_api_gateway_resource.users.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_method" "get_user" {
  rest_api_id   = aws_api_gateway_rest_api.users_api.id
  resource_id   = aws_api_gateway_resource.user_by_id.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "list_users" {
  rest_api_id             = aws_api_gateway_rest_api.users_api.id
  resource_id             = aws_api_gateway_resource.users.id
  http_method             = aws_api_gateway_method.list_users.http_method
  type                    = "AWS_PROXY"
  integration_http_method = "POST"
  uri                     = aws_lambda_function.list_users.invoke_arn
}

resource "aws_api_gateway_integration" "get_user" {
  rest_api_id             = aws_api_gateway_rest_api.users_api.id
  resource_id             = aws_api_gateway_resource.user_by_id.id
  http_method             = aws_api_gateway_method.get_user.http_method
  type                    = "AWS_PROXY"
  integration_http_method = "POST"
  uri                     = aws_lambda_function.get_user.invoke_arn
}

# ===== Products API (HTTP v2) =====
resource "aws_apigatewayv2_api" "products_api" {
  name          = "${local.prefix}-products-api"
  protocol_type = "HTTP"
}

resource "aws_lambda_function" "list_products" {
  function_name = "${local.prefix}-list-products"
  handler       = "handler.list_products"
  runtime       = "python3.12"
  filename      = "services/products/handler.py"
  role          = "arn:aws:iam::123456789012:role/lambda-role"
  timeout       = 15

  environment {
    variables = {
      TABLE_NAME = "products-${var.environment}"
    }
  }
}

resource "aws_lambda_function" "get_product" {
  function_name = "${local.prefix}-get-product"
  handler       = "handler.get_product"
  runtime       = "python3.12"
  filename      = "services/products/handler.py"
  role          = "arn:aws:iam::123456789012:role/lambda-role"
  timeout       = 15

  environment {
    variables = {
      TABLE_NAME = "products-${var.environment}"
    }
  }
}

resource "aws_apigatewayv2_integration" "list_products" {
  api_id                 = aws_apigatewayv2_api.products_api.id
  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.list_products.invoke_arn
  payload_format_version = "2.0"
}

resource "aws_apigatewayv2_integration" "get_product" {
  api_id                 = aws_apigatewayv2_api.products_api.id
  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.get_product.invoke_arn
  payload_format_version = "2.0"
}

resource "aws_apigatewayv2_route" "list_products" {
  api_id    = aws_apigatewayv2_api.products_api.id
  route_key = "GET /products"
  target    = "integrations/${aws_apigatewayv2_integration.list_products.id}"
}

resource "aws_apigatewayv2_route" "get_product" {
  api_id    = aws_apigatewayv2_api.products_api.id
  route_key = "GET /products/{productId}"
  target    = "integrations/${aws_apigatewayv2_integration.get_product.id}"
}

# ===== Orders API (REST v1, different service) =====
resource "aws_api_gateway_rest_api" "orders_api" {
  name = "${local.prefix}-orders-api"
}

resource "aws_lambda_function" "create_order" {
  function_name = "${local.prefix}-create-order"
  handler       = "handler.create_order"
  runtime       = "python3.12"
  filename      = "services/orders/handler.py"
  role          = "arn:aws:iam::123456789012:role/lambda-role"
  timeout       = 30

  environment {
    variables = {
      USERS_TABLE    = "users-${var.environment}"
      PRODUCTS_TABLE = "products-${var.environment}"
      ORDERS_TABLE   = "orders-${var.environment}"
    }
  }
}

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
