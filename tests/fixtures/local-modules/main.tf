variable "environment" {
  default = "dev"
}

module "api" {
  source = "./modules/api"

  environment = var.environment
  table_name  = "users-table"
}

resource "aws_lambda_function" "root_handler" {
  function_name = "root-handler"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  timeout       = 10
}
