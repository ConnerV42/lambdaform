# Terraform Modules — Nested Module Discovery

Demonstrates Lambdaform's ability to discover Lambda functions across **3 levels of nested Terraform modules**.

## Structure

```
terraform-modules/
├── main.tf                    # Root module (root_handler + API Gateway)
├── root/index.js              # Root-level handler
└── modules/
    ├── shared/                # Level 1: shared layer module
    │   └── main.tf
    └── api/                   # Level 1: API module
        ├── main.tf            # api_handler
        └── routes/
            └── health/        # Level 2: health route sub-module
                └── main.tf    # health_handler
```

## What This Tests

- Recursive module discovery (root → modules/api → modules/api/routes/health)
- Function name prefixing with module path
- Cross-module references (shared layer ARN passed to API module)
- Mixed root-level and module-level functions in one project

## Run It

```bash
cd examples/terraform-modules
lambdaform start
```

You should see all 4 functions discovered:

```
Discovered functions:
  root_handler            (nodejs20.x)
  shared.utils            (nodejs20.x)
  api.api_handler         (nodejs20.x)
  api.health.health_check (nodejs20.x)
```

## Test

```bash
# Root-level function
curl http://localhost:3000/status

# Functions from nested modules can be invoked directly
lambdaform invoke --function api.api_handler --payload '{}'
lambdaform invoke --function api.health.health_check --payload '{}'
```
