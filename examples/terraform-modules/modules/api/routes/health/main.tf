# Health routes module (depth 3) — deepest nesting level

variable "environment" {}
variable "project_name" {}
variable "prefix" {}

resource "aws_lambda_function" "health_check" {
  function_name = "${var.prefix}-health"
  handler       = "health.handler"
  runtime       = "python3.12"
  filename      = "health.py"
  role          = "arn:aws:iam::123456789012:role/lambda-role"
  timeout       = 5

  environment {
    variables = {
      SERVICE_NAME = var.project_name
      ENV          = var.environment
    }
  }
}

resource "aws_lambda_function" "health_detailed" {
  function_name = "${var.prefix}-health-detailed"
  handler       = "detailed.handler"
  runtime       = "python3.12"
  filename      = "detailed.py"
  role          = "arn:aws:iam::123456789012:role/lambda-role"
  timeout       = 10

  environment {
    variables = {
      SERVICE_NAME = var.project_name
      ENV          = var.environment
      VERSION      = "1.0.0"
    }
  }
}
