# Authorizer Flow Example
# Demonstrates API Gateway with Lambda TOKEN authorizer

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

# --- IAM ---

resource "aws_iam_role" "lambda_role" {
  name = "authorizer-flow-lambda-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "lambda.amazonaws.com" }
    }]
  })
}

# --- Lambda Functions ---

resource "aws_lambda_function" "authorizer" {
  function_name = "token-authorizer"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda_role.arn
  filename      = "src/auth/index.js"
  source_code_hash = filebase64sha256("src/auth/index.js")

  environment {
    variables = {
      AUTH_TOKEN = "super-secret-token-123"
    }
  }
}

resource "aws_lambda_function" "protected_api" {
  function_name = "protected-api"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda_role.arn
  filename      = "src/api/index.js"
  source_code_hash = filebase64sha256("src/api/index.js")
}

resource "aws_lambda_function" "public_api" {
  function_name = "public-api"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda_role.arn
  filename      = "src/api/index.js"
  source_code_hash = filebase64sha256("src/api/index.js")
}

# --- API Gateway ---

resource "aws_api_gateway_rest_api" "api" {
  name        = "authorizer-flow-api"
  description = "API with Lambda authorizer"
}

# Authorizer
resource "aws_api_gateway_authorizer" "token_auth" {
  name                   = "token-authorizer"
  rest_api_id            = aws_api_gateway_rest_api.api.id
  type                   = "TOKEN"
  authorizer_uri         = aws_lambda_function.authorizer.invoke_arn
  identity_source        = "method.request.header.Authorization"
}

# /public (no auth)
resource "aws_api_gateway_resource" "public" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "public"
}

resource "aws_api_gateway_method" "public_get" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.public.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "public_get" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.public.id
  http_method             = aws_api_gateway_method.public_get.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.public_api.invoke_arn
}

# /protected (with auth)
resource "aws_api_gateway_resource" "protected" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "protected"
}

resource "aws_api_gateway_method" "protected_get" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.protected.id
  http_method   = "GET"
  authorization = "CUSTOM"
  authorizer_id = aws_api_gateway_authorizer.token_auth.id
}

resource "aws_api_gateway_integration" "protected_get" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.protected.id
  http_method             = aws_api_gateway_method.protected_get.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.protected_api.invoke_arn
}

# /protected POST (also auth-protected)
resource "aws_api_gateway_method" "protected_post" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.protected.id
  http_method   = "POST"
  authorization = "CUSTOM"
  authorizer_id = aws_api_gateway_authorizer.token_auth.id
}

resource "aws_api_gateway_integration" "protected_post" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.protected.id
  http_method             = aws_api_gateway_method.protected_post.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.protected_api.invoke_arn
}
