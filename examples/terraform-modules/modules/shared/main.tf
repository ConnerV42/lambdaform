# Shared module — provides a Lambda layer used by other modules

variable "environment" {}
variable "project_name" {}

locals {
  layer_name = "${var.project_name}-${var.environment}-shared-utils"
}

resource "aws_lambda_layer_version" "shared_utils" {
  layer_name          = local.layer_name
  filename            = "layer/utils.js"
  compatible_runtimes = ["nodejs20.x"]
}

output "layer_arn" {
  value = aws_lambda_layer_version.shared_utils.arn
}
