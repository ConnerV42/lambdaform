# Environment Variables

## Variables Set by Lambdaform

These are injected into every Lambda invocation:

| Variable | Value | Description |
|----------|-------|-------------|
| `AWS_LAMBDA_FUNCTION_NAME` | Function name | From Terraform `function_name` |
| `AWS_LAMBDA_FUNCTION_VERSION` | `$LATEST` | Always `$LATEST` locally |
| `_HANDLER` | Handler path | From Terraform `handler` |
| `LAMBDA_TASK_ROOT` | Source directory | Resolved function source path |
| `AWS_REGION` | `us-east-1` | Default (overridable via config) |
| `AWS_DEFAULT_REGION` | `us-east-1` | Default (overridable via config) |

## User Variables

### From Terraform

```hcl
resource "aws_lambda_function" "api" {
  environment {
    variables = {
      TABLE_NAME = "users"
      API_KEY    = "local-key"
    }
  }
}
```

### From `lambdaform.yaml`

Global (all functions):
```yaml
environment:
  STAGE: local
```

Per-function:
```yaml
functions:
  api:
    environment:
      TABLE_NAME: local-users
```

### Precedence

From highest to lowest priority:
1. Per-function config (`functions.<name>.environment`)
2. Global config (`environment`)
3. Terraform (`environment.variables`)
4. Lambdaform defaults (`AWS_LAMBDA_FUNCTION_NAME`, etc.)
