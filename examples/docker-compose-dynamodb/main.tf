provider "aws" {
  region = "us-east-1"
}

# --- DynamoDB Table ---

resource "aws_dynamodb_table" "items" {
  name         = "items"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }
}

# --- Lambda Function ---

resource "aws_lambda_function" "api" {
  function_name = "items-api"
  runtime       = "nodejs20.x"
  handler       = "index.handler"
  filename      = "lambda.zip"
  role          = aws_iam_role.lambda.arn

  environment {
    variables = {
      TABLE_NAME       = aws_dynamodb_table.items.name
      AWS_ENDPOINT_URL = "http://localhost:8000"
    }
  }
}

# --- API Gateway ---

resource "aws_apigatewayv2_api" "api" {
  name          = "items-api"
  protocol_type = "HTTP"
}

resource "aws_apigatewayv2_integration" "api" {
  api_id             = aws_apigatewayv2_api.api.id
  integration_type   = "AWS_PROXY"
  integration_uri    = aws_lambda_function.api.invoke_arn
  integration_method = "POST"
}

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

resource "aws_apigatewayv2_route" "delete_item" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "DELETE /items/{id}"
  target    = "integrations/${aws_apigatewayv2_integration.api.id}"
}

# --- IAM (required by Terraform, not used locally) ---

resource "aws_iam_role" "lambda" {
  name               = "items-api-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "lambda.amazonaws.com" }
    }]
  })
}
