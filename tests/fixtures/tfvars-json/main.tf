# Test fixture for .tfvars.json format

variable "environment" {
  type    = string
  default = "dev"
}

variable "region" {
  type    = string
  default = "us-east-1"
}

variable "timeout" {
  type    = number
  default = 30
}

resource "aws_lambda_function" "app" {
  function_name = "${var.environment}-app"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  timeout       = var.timeout
  filename      = "app.zip"

  environment {
    variables = {
      ENV    = var.environment
      REGION = var.region
    }
  }
}
