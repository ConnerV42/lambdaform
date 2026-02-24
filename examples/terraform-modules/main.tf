# Root module — uses nested modules 3 levels deep
# Root -> modules/api -> modules/api/routes/health

variable "environment" {
  default = "dev"
}

variable "project_name" {
  default = "nested-modules-demo"
}

locals {
  app_prefix = "${var.project_name}-${var.environment}"
}

module "shared" {
  source = "./modules/shared"

  environment = var.environment
  project_name = var.project_name
}

module "api" {
  source = "./modules/api"

  environment  = var.environment
  project_name = var.project_name
  shared_layer_arn = module.shared.layer_arn
}

# Root-level function to test mixed root + module functions
resource "aws_lambda_function" "root_handler" {
  function_name = "${local.app_prefix}-root"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "root/index.js"
  role          = "arn:aws:iam::123456789012:role/lambda-role"
  timeout       = 10

  environment {
    variables = {
      ENV = var.environment
    }
  }
}

resource "aws_api_gateway_rest_api" "main" {
  name = "${local.app_prefix}-api"
}

resource "aws_api_gateway_resource" "root_resource" {
  rest_api_id = aws_api_gateway_rest_api.main.id
  parent_id   = aws_api_gateway_rest_api.main.root_resource_id
  path_part   = "status"
}

resource "aws_api_gateway_method" "root_get" {
  rest_api_id   = aws_api_gateway_rest_api.main.id
  resource_id   = aws_api_gateway_resource.root_resource.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "root_integration" {
  rest_api_id             = aws_api_gateway_rest_api.main.id
  resource_id             = aws_api_gateway_resource.root_resource.id
  http_method             = aws_api_gateway_method.root_get.http_method
  type                    = "AWS_PROXY"
  integration_http_method = "POST"
  uri                     = aws_lambda_function.root_handler.invoke_arn
}
