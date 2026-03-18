# Lambda Function URLs Example
#
# Demonstrates Lambda Function URLs — direct HTTPS endpoints for Lambda
# functions without API Gateway. Each function gets its own URL.
#
# Usage:
#   lambdaform start
#   curl http://localhost:3001/      # greeting function
#   curl http://localhost:3002/      # echo function (returns request details)

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

# IAM role for Lambda execution
resource "aws_iam_role" "lambda_role" {
  name = "function-urls-example-role"

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

# --- Greeting Function ---

resource "aws_lambda_function" "greeting" {
  function_name = "greeting"
  handler       = "greeting.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda_role.arn
  filename      = "greeting.zip"

  environment {
    variables = {
      APP_NAME = "Function URLs Demo"
    }
  }
}

resource "aws_lambda_function_url" "greeting_url" {
  function_name      = aws_lambda_function.greeting.function_name
  authorization_type = "NONE"

  cors {
    allow_origins     = ["*"]
    allow_methods     = ["GET", "POST"]
    allow_headers     = ["Content-Type"]
    max_age           = 86400
  }
}

# --- Echo Function ---

resource "aws_lambda_function" "echo" {
  function_name = "echo"
  handler       = "echo.handler"
  runtime       = "python3.12"
  role          = aws_iam_role.lambda_role.arn
  filename      = "echo.zip"
}

resource "aws_lambda_function_url" "echo_url" {
  function_name      = aws_lambda_function.echo.function_name
  authorization_type = "AWS_IAM"

  cors {
    allow_origins     = ["https://example.com"]
    allow_methods     = ["GET", "POST", "PUT", "DELETE"]
    allow_headers     = ["Content-Type", "Authorization"]
    allow_credentials = true
    max_age           = 3600
  }
}

# Outputs
output "greeting_url" {
  value = aws_lambda_function_url.greeting_url.function_url
}

output "echo_url" {
  value = aws_lambda_function_url.echo_url.function_url
}
