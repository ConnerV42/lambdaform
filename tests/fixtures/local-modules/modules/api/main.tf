variable "environment" {
  default = "prod"
}

variable "table_name" {
  default = "default-table"
}

resource "aws_lambda_function" "api_handler" {
  function_name = "${var.environment}-api-handler"
  handler       = "app.handler"
  runtime       = "python3.12"
  timeout       = 30

  environment {
    variables = {
      TABLE_NAME  = var.table_name
      ENVIRONMENT = var.environment
    }
  }
}

resource "aws_dynamodb_table" "data" {
  name     = "${var.environment}-data"
  hash_key = "id"

  billing_mode = "PAY_PER_REQUEST"
}
