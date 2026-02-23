terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

provider "aws" {
  region = "us-west-2"
}

locals {
  api_version = "v1"
  environment = "development"
}

# Lambda IAM Role (shared by all functions)
resource "aws_iam_role" "lambda_role" {
  name = "multi-function-lambda-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRole"
      Effect = "Allow"
      Principal = {
        Service = "lambda.amazonaws.com"
      }
    }]
  })
}

resource "aws_iam_role_policy_attachment" "lambda_basic" {
  role       = aws_iam_role.lambda_role.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

# Shared layer with common utilities
resource "aws_lambda_layer_version" "common_utils" {
  filename            = "layers/common-utils"
  layer_name          = "common-utils"
  compatible_runtimes = ["nodejs20.x"]
}

# User Service Function
resource "aws_lambda_function" "user_service" {
  filename         = "user.zip"
  function_name    = "user-service"
  role            = aws_iam_role.lambda_role.arn
  handler         = "user.handler"
  source_code_hash = filebase64sha256("user.zip")
  runtime         = "nodejs20.x"
  timeout         = 30
  layers          = [aws_lambda_layer_version.common_utils.arn]

  environment {
    variables = {
      API_VERSION  = local.api_version
      SERVICE_NAME = "user-service"
      ENVIRONMENT  = local.environment
      LOG_LEVEL    = "info"
    }
  }
}

# Order Service Function
resource "aws_lambda_function" "order_service" {
  filename         = "order.zip"
  function_name    = "order-service"
  role            = aws_iam_role.lambda_role.arn
  handler         = "order.handler"
  source_code_hash = filebase64sha256("order.zip")
  runtime         = "nodejs20.x"
  timeout         = 30
  layers          = [aws_lambda_layer_version.common_utils.arn]

  environment {
    variables = {
      API_VERSION  = local.api_version
      SERVICE_NAME = "order-service"
      ENVIRONMENT  = local.environment
      LOG_LEVEL    = "info"
      MAX_ORDERS   = "100"
    }
  }
}

# Notification Service Function
resource "aws_lambda_function" "notification_service" {
  filename         = "notification.zip"
  function_name    = "notification-service"
  role            = aws_iam_role.lambda_role.arn
  handler         = "notification.handler"
  source_code_hash = filebase64sha256("notification.zip")
  runtime         = "nodejs20.x"
  timeout         = 30
  layers          = [aws_lambda_layer_version.common_utils.arn]

  environment {
    variables = {
      API_VERSION    = local.api_version
      SERVICE_NAME   = "notification-service"
      ENVIRONMENT    = local.environment
      LOG_LEVEL      = "debug"
      EMAIL_ENABLED  = "true"
      SMS_ENABLED    = "false"
    }
  }
}

# API Gateway REST API
resource "aws_api_gateway_rest_api" "api" {
  name        = "multi-function-api"
  description = "Multi-function API with shared layers"
}

# /users resource
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
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.users.id
  http_method             = aws_api_gateway_method.get_users.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.user_service.invoke_arn
}

# /orders resource
resource "aws_api_gateway_resource" "orders" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "orders"
}

resource "aws_api_gateway_method" "get_orders" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.orders.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "get_orders" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.orders.id
  http_method             = aws_api_gateway_method.get_orders.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.order_service.invoke_arn
}

# /notifications resource
resource "aws_api_gateway_resource" "notifications" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "notifications"
}

resource "aws_api_gateway_method" "post_notifications" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.notifications.id
  http_method   = "POST"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "post_notifications" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.notifications.id
  http_method             = aws_api_gateway_method.post_notifications.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.notification_service.invoke_arn
}

# Lambda permissions
resource "aws_lambda_permission" "user_api_gateway" {
  statement_id  = "AllowAPIGatewayInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.user_service.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_api_gateway_rest_api.api.execution_arn}/*/*"
}

resource "aws_lambda_permission" "order_api_gateway" {
  statement_id  = "AllowAPIGatewayInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.order_service.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_api_gateway_rest_api.api.execution_arn}/*/*"
}

resource "aws_lambda_permission" "notification_api_gateway" {
  statement_id  = "AllowAPIGatewayInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.notification_service.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_api_gateway_rest_api.api.execution_arn}/*/*"
}

# Deployment
resource "aws_api_gateway_deployment" "api" {
  depends_on = [
    aws_api_gateway_integration.get_users,
    aws_api_gateway_integration.get_orders,
    aws_api_gateway_integration.post_notifications,
  ]

  rest_api_id = aws_api_gateway_rest_api.api.id
  stage_name  = "dev"
}

# Outputs
output "api_url" {
  value = "${aws_api_gateway_deployment.api.invoke_url}"
}
