variable "env" {}
variable "app" {}

locals {
  prefix = "${var.app}-${var.env}"
}

resource "aws_lambda_function" "worker" {
  function_name = "${local.prefix}-worker"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "worker.js"
  role          = "arn:aws:iam::123456789012:role/role"

  environment {
    variables = {
      PREFIX = local.prefix
    }
  }
}
