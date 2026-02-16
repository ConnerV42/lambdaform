# Terraform Variables

Lambdaform resolves Terraform variables and locals so your infrastructure configuration works without running `terraform plan`.

## Variable Resolution

### From `.tfvars` files

```bash
lambdaform start --var-file=dev.tfvars
lambdaform start --var-file=dev.tfvars --var-file=overrides.tfvars
```

`terraform.tfvars` is loaded automatically if present.

### Default values

Variables with `default` in their declaration are resolved:

```hcl
variable "stage" {
  default = "dev"
}

resource "aws_lambda_function" "api" {
  function_name = "api-${var.stage}"    # resolves to "api-dev"
}
```

### Interpolation

Lambdaform resolves `${var.name}` and `${local.name}` expressions in string values:

```hcl
variable "region" { default = "us-west-2" }

locals {
  prefix     = "myapp-${var.stage}"
  table_name = "${local.prefix}-users"
}

resource "aws_lambda_function" "api" {
  environment {
    variables = {
      TABLE_NAME = local.table_name    # resolves to "myapp-dev-users"
      REGION     = var.region           # resolves to "us-west-2"
    }
  }
}
```

### Cross-references

Locals can reference other locals. Lambdaform resolves them iteratively:

```hcl
locals {
  base   = "myapp"
  prefix = "${local.base}-${var.stage}"
  name   = "${local.prefix}-api"        # "myapp-dev-api"
}
```

## Limitations

- **Complex expressions** (conditionals, `for` loops, `lookup()`) are not evaluated — use `lambdaform.yaml` overrides for these
- **Data sources** and **remote state** references are not resolved
- **Module variables** passed via `module.x.output` are resolved when using local modules
