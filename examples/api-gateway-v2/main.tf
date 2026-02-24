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

# Lambda IAM Role
resource "aws_iam_role" "lambda_role" {
  name = "api-v2-lambda-role"

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

# Lambda Function — handles all routes
resource "aws_lambda_function" "api" {
  filename         = "function.zip"
  function_name    = "api-v2-handler"
  role             = aws_iam_role.lambda_role.arn
  handler          = "index.handler"
  source_code_hash = filebase64sha256("function.zip")
  runtime          = "nodejs20.x"
  timeout          = 30

  environment {
    variables = {
      NODE_ENV   = "production"
      TABLE_NAME = "items"
    }
  }
}

# HTTP API (API Gateway v2)
resource "aws_apigatewayv2_api" "api" {
  name          = "api-v2-example"
  protocol_type = "HTTP"

  cors_configuration {
    allow_origins = ["*"]
    allow_methods = ["GET", "POST", "PUT", "DELETE", "OPTIONS"]
    allow_headers = ["Content-Type", "Authorization"]
    max_age       = 3600
  }
}

# Integration — connects API to Lambda
resource "aws_apigatewayv2_integration" "api" {
  api_id                 = aws_apigatewayv2_api.api.id
  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.api.invoke_arn
  payload_format_version = "2.0"
}

# Routes
resource "aws_apigatewayv2_route" "get_items" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "GET /items"
  target    = "integrations/${aws_apigatewayv2_integration.api.id}"
}

resource "aws_apigatewayv2_route" "post_items" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "POST /items"
  target    = "integrations/${aws_apigatewayv2_integration.api.id}"
}

resource "aws_apigatewayv2_route" "get_item" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "GET /items/{id}"
  target    = "integrations/${aws_apigatewayv2_integration.api.id}"
}

resource "aws_apigatewayv2_route" "put_item" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "PUT /items/{id}"
  target    = "integrations/${aws_apigatewayv2_integration.api.id}"
}

resource "aws_apigatewayv2_route" "delete_item" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "DELETE /items/{id}"
  target    = "integrations/${aws_apigatewayv2_integration.api.id}"
}

# Health check route — different function
resource "aws_lambda_function" "health" {
  filename         = "function.zip"
  function_name    = "api-v2-health"
  role             = aws_iam_role.lambda_role.arn
  handler          = "health.handler"
  source_code_hash = filebase64sha256("function.zip")
  runtime          = "nodejs20.x"
  timeout          = 5
}

resource "aws_apigatewayv2_integration" "health" {
  api_id                 = aws_apigatewayv2_api.api.id
  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.health.invoke_arn
  payload_format_version = "2.0"
}

resource "aws_apigatewayv2_route" "health" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "GET /health"
  target    = "integrations/${aws_apigatewayv2_integration.health.id}"
}

# Stage
resource "aws_apigatewayv2_stage" "dev" {
  api_id      = aws_apigatewayv2_api.api.id
  name        = "$default"
  auto_deploy = true
}

# Lambda permissions
resource "aws_lambda_permission" "api" {
  statement_id  = "AllowHTTPAPIInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.api.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_apigatewayv2_api.api.execution_arn}/*/*"
}

resource "aws_lambda_permission" "health" {
  statement_id  = "AllowHTTPAPIInvokeHealth"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.health.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_apigatewayv2_api.api.execution_arn}/*/*"
}
