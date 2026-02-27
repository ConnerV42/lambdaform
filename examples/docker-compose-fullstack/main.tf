provider "aws" {
  region = "us-east-1"
}

# --- DynamoDB ---

resource "aws_dynamodb_table" "uploads" {
  name         = "uploads"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }
}

# --- S3 ---

resource "aws_s3_bucket" "uploads" {
  bucket = "uploads-bucket"
}

# --- Lambda Functions ---

resource "aws_lambda_function" "upload" {
  function_name = "upload-handler"
  runtime       = "python3.12"
  handler       = "upload.handler"
  filename      = "lambda.zip"
  role          = aws_iam_role.lambda.arn
  timeout       = 30

  environment {
    variables = {
      TABLE_NAME     = aws_dynamodb_table.uploads.name
      BUCKET_NAME    = aws_s3_bucket.uploads.id
      DYNAMODB_URL   = "http://localhost:8000"
      S3_URL         = "http://localhost:4566"
    }
  }
}

resource "aws_lambda_function" "list" {
  function_name = "list-handler"
  runtime       = "python3.12"
  handler       = "list_uploads.handler"
  filename      = "lambda.zip"
  role          = aws_iam_role.lambda.arn

  environment {
    variables = {
      TABLE_NAME   = aws_dynamodb_table.uploads.name
      DYNAMODB_URL = "http://localhost:8000"
    }
  }
}

# --- API Gateway v2 ---

resource "aws_apigatewayv2_api" "api" {
  name          = "uploads-api"
  protocol_type = "HTTP"
}

resource "aws_apigatewayv2_integration" "upload" {
  api_id             = aws_apigatewayv2_api.api.id
  integration_type   = "AWS_PROXY"
  integration_uri    = aws_lambda_function.upload.invoke_arn
  integration_method = "POST"
}

resource "aws_apigatewayv2_integration" "list" {
  api_id             = aws_apigatewayv2_api.api.id
  integration_type   = "AWS_PROXY"
  integration_uri    = aws_lambda_function.list.invoke_arn
  integration_method = "POST"
}

resource "aws_apigatewayv2_route" "upload" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "POST /uploads"
  target    = "integrations/${aws_apigatewayv2_integration.upload.id}"
}

resource "aws_apigatewayv2_route" "list" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "GET /uploads"
  target    = "integrations/${aws_apigatewayv2_integration.list.id}"
}

resource "aws_apigatewayv2_route" "get" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "GET /uploads/{id}"
  target    = "integrations/${aws_apigatewayv2_integration.list.id}"
}

# --- IAM ---

resource "aws_iam_role" "lambda" {
  name               = "uploads-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "lambda.amazonaws.com" }
    }]
  })
}
