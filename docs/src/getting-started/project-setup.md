# Project Setup

## Using `lambdaform init`

The `init` command detects your project structure and generates a `lambdaform.yaml` config file:

```bash
lambdaform init
```

It auto-detects:
- Existing `.tf` files and their location
- Lambda runtimes in use (Node.js, Python, Go)
- API Gateway resources
- Whether you're using Terraform or OpenTofu

For non-interactive environments:
```bash
lambdaform init --yes    # accept all defaults
```

## Project Structure

Lambdaform works with any directory layout. It recursively scans for `.tf` files starting from the target directory.

### Simple project
```
my-app/
├── main.tf              # Lambda + API Gateway resources
├── variables.tf         # Terraform variables
├── terraform.tfvars     # Variable values
├── index.js             # Lambda handler
└── lambdaform.yaml      # Optional config overrides
```

### Monorepo with modules
```
infra/
├── main.tf
├── modules/
│   ├── api/
│   │   └── main.tf
│   └── functions/
│       └── main.tf
├── src/
│   ├── handler-a/
│   │   └── index.js
│   └── handler-b/
│       └── handler.py
└── lambdaform.yaml
```

Run from the infra directory:
```bash
cd infra
lambdaform start
```

Or specify the directory:
```bash
lambdaform start --dir ./infra
```

## Validate Your Setup

Before starting the server, validate that Lambdaform can parse your Terraform:

```bash
lambdaform validate
```

```
🔍 Validating Terraform in: ./
   Found 3 .tf file(s)
   Found 5 function(s), 2 gateway(s), 8 route(s)
✅ Validation passed!
```

If validation reports issues, check:
1. All `aws_lambda_function` resources have `handler` and `runtime` attributes
2. API Gateway integrations reference Lambda functions via `invoke_arn` or `arn`
3. `.tf` files are valid HCL syntax

## Using with `.tfvars`

Lambdaform resolves Terraform variables from `.tfvars` files:

```bash
lambdaform start --var-file=dev.tfvars
lambdaform start --var-file=dev.tfvars --var-file=overrides.tfvars
```

It automatically loads `terraform.tfvars` if present. See [Terraform Variables](../guide/variables.md) for details.
