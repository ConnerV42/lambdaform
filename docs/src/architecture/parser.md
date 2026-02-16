# HCL Parser

Lambdaform includes a custom HCL parser (`parser.rs`, 2,780 lines) that extracts AWS resource definitions from `.tf` files without shelling out to Terraform.

## What It Parses

### Resources
- `aws_lambda_function` — handler, runtime, timeout, memory, environment, layers
- `aws_api_gateway_rest_api` — REST API (v1)
- `aws_api_gateway_resource` — path segments and parent chains
- `aws_api_gateway_method` — HTTP method bindings
- `aws_api_gateway_integration` — Lambda proxy integrations
- `aws_api_gateway_authorizer` — TOKEN/REQUEST authorizers
- `aws_apigatewayv2_api` — HTTP API (v2) and WebSocket API
- `aws_apigatewayv2_route` — route keys
- `aws_apigatewayv2_integration` — Lambda integrations
- `aws_apigatewayv2_authorizer` — v2 authorizers
- `aws_lambda_layer_version` — layer paths
- `aws_lambda_event_source_mapping` — SQS/SNS triggers
- `aws_sqs_queue`, `aws_sns_topic` — queue/topic metadata
- `aws_sfn_state_machine` — Step Functions definitions
- `aws_dynamodb_table` — table schema (for hints)

### Declarations
- `variable` — with defaults and `.tfvars` resolution
- `locals` — with cross-reference resolution
- `module` — local source following

## Reference Resolution

The parser resolves Terraform reference expressions like:
- `aws_lambda_function.handler.invoke_arn` → maps integration to function
- `aws_api_gateway_rest_api.api.root_resource_id` → identifies API root
- `aws_api_gateway_resource.parent.id` → builds path trees

This is pattern-based, not a full Terraform expression evaluator. It handles the common patterns used in Lambda + API Gateway configurations.

## Error Reporting

Parse errors include source location (file, line, column):

```
Error: unexpected token at main.tf:42:15
  expected closing brace, found "resource"
```

## Limitations

- **Not a complete HCL parser** — handles the subset needed for Lambda/APIGW resources
- **Complex expressions** (`for`, `lookup`, conditionals) are not evaluated
- **Dynamic blocks** are not expanded
- **Provider-specific functions** (`cidrsubnet`, `base64encode`, etc.) are not executed
