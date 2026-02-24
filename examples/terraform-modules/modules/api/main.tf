# API module (depth 2) — defines Lambda functions and delegates to sub-modules

variable "environment" {}
variable "project_name" {}
variable "shared_layer_arn" {}

locals {
  prefix = "${var.project_name}-${var.environment}-api"
}

# Direct function in this module
resource "aws_lambda_function" "list_items" {
  function_name = "${local.prefix}-list-items"
  handler       = "list.handler"
  runtime       = "nodejs20.x"
  filename      = "handlers/list.js"
  role          = "arn:aws:iam::123456789012:role/lambda-role"
  timeout       = 15
  layers        = [var.shared_layer_arn]

  environment {
    variables = {
      TABLE_NAME = "items-${var.environment}"
      ENV        = var.environment
    }
  }
}

# Depth-3 sub-module for health routes
module "health" {
  source = "./routes/health"

  environment  = var.environment
  project_name = var.project_name
  prefix       = local.prefix
}
