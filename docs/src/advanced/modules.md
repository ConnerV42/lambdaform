# Terraform Modules

Lambdaform resolves local Terraform modules, scanning into module directories to discover Lambda functions and API Gateway resources.

## How It Works

When Lambdaform encounters a `module` block with a local `source`, it follows the path and parses the module's `.tf` files:

```hcl
module "api" {
  source = "./modules/api"

  stage       = var.stage
  table_name  = aws_dynamodb_table.users.name
}
```

Resources inside `./modules/api/` are discovered and included in the route table.

## Variable Passing

Module input variables are resolved from the calling module's arguments:

```hcl
# modules/api/variables.tf
variable "stage" {}
variable "table_name" {}

# modules/api/main.tf
resource "aws_lambda_function" "handler" {
  function_name = "api-${var.stage}"
  environment {
    variables = {
      TABLE_NAME = var.table_name
    }
  }
}
```

## Limitations

- **Only local modules** (`source = "./path"`) are supported
- **Registry modules** (`source = "terraform-aws-modules/..."`) are not fetched
- **Remote modules** (git, S3, etc.) are not fetched
- **Module outputs** referenced across modules may not resolve in all cases

For unsupported module patterns, use `lambdaform.yaml` overrides to fill in missing values.
