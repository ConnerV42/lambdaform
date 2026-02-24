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

# IAM role for Lambda
resource "aws_iam_role" "lambda" {
  name = "large-payload-lambda-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "lambda.amazonaws.com" }
    }]
  })
}

# --- Text echo function (handles JSON payloads) ---
resource "aws_lambda_function" "text_echo" {
  function_name = "text-echo"
  handler       = "handler.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda.arn
  filename      = "text-echo.zip"
  timeout       = 30
  memory_size   = 256
}

# --- Binary handler (receives base64-encoded binary, returns binary) ---
resource "aws_lambda_function" "binary_handler" {
  function_name = "binary-handler"
  handler       = "binary.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda.arn
  filename      = "binary-handler.zip"
  timeout       = 30
  memory_size   = 256
}

# --- Image processor (accepts image upload, returns metadata) ---
resource "aws_lambda_function" "image_processor" {
  function_name = "image-processor"
  handler       = "image.handler"
  runtime       = "python3.12"
  role          = aws_iam_role.lambda.arn
  filename      = "image-processor.zip"
  timeout       = 30
  memory_size   = 256
}

# REST API Gateway (v1) — 10MB limit
resource "aws_api_gateway_rest_api" "api" {
  name = "large-payload-api"
  binary_media_types = [
    "application/octet-stream",
    "image/*",
    "multipart/form-data"
  ]
}

resource "aws_api_gateway_resource" "echo" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "echo"
}

resource "aws_api_gateway_method" "echo_post" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.echo.id
  http_method   = "POST"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "echo_post" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.echo.id
  http_method             = aws_api_gateway_method.echo_post.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.text_echo.invoke_arn
}

resource "aws_api_gateway_resource" "binary" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "binary"
}

resource "aws_api_gateway_method" "binary_post" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.binary.id
  http_method   = "POST"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "binary_post" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.binary.id
  http_method             = aws_api_gateway_method.binary_post.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.binary_handler.invoke_arn
}

resource "aws_api_gateway_resource" "image" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "image"
}

resource "aws_api_gateway_method" "image_post" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.image.id
  http_method   = "POST"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "image_post" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.image.id
  http_method             = aws_api_gateway_method.image_post.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.image_processor.invoke_arn
}

# Function URL for binary_handler (6MB limit)
resource "aws_lambda_function_url" "binary_url" {
  function_name      = aws_lambda_function.binary_handler.function_name
  authorization_type = "NONE"
}
