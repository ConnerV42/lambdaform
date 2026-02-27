# FAQ

## General

### Does Lambdaform replace Terraform?

No. Lambdaform is a development tool that *reads* your Terraform configuration to run Lambda functions locally. You still use Terraform (or OpenTofu) to deploy to AWS. Think of it as a companion tool, like how `webpack-dev-server` doesn't replace webpack.

### Does Lambdaform need AWS credentials?

No. Lambdaform runs everything locally. No AWS API calls are made. If your Lambda *code* needs AWS services (DynamoDB, S3, etc.), you'll need credentials for those — but Lambdaform itself doesn't.

### Does it work with OpenTofu?

Yes. Lambdaform parses `.tf` files directly and works identically with both Terraform and OpenTofu projects. See [OpenTofu Compatibility](../advanced/opentofu.md).

### What Terraform providers are supported?

Lambdaform understands resources from the `aws` provider:
- `aws_lambda_function`
- `aws_api_gateway_rest_api` (and related resources)
- `aws_apigatewayv2_api` (HTTP and WebSocket APIs)
- `aws_lambda_layer_version`
- `aws_sqs_queue`, `aws_sns_topic`
- `aws_dynamodb_table`
- `aws_sfn_state_machine`
- `aws_lambda_function_url`
- And more

Other providers' resources are silently ignored.

### Is Docker required?

Only for Java runtimes (`java8.al2`, `java11`, `java17`, `java21`), which use AWS Lambda base Docker images for JVM parity. All other runtimes (Node.js, Python, Go, Rust) run natively without Docker.

## Features

### Can I test multiple Lambda functions at once?

Yes. Lambdaform starts all discovered functions and routes requests based on your API Gateway configuration. Each function gets its own warm worker process.

### Does hot reload work with Terraform changes?

Yes. Lambdaform watches both your source code files *and* `.tf` files. When you modify a Terraform file (add a route, change a handler), the server automatically reloads.

### Can I use Lambda layers?

Yes. Lambdaform resolves `aws_lambda_layer_version` resources and makes their contents available to functions. For Node.js, layer paths are added to `NODE_PATH`. For Python, they're added to `PYTHONPATH`. See [Lambda Layers](./layers.md).

### Does it support Lambda Function URLs?

Yes. Functions with `aws_lambda_function_url` resources get their own dedicated HTTP endpoint, separate from API Gateway routes.

### Can I simulate SQS/SNS triggers?

Yes. Use `lambdaform trigger` to send test events:

```bash
# SQS trigger
lambdaform trigger --function my-processor --source-type sqs --body '{"key": "value"}'

# SNS trigger
lambdaform trigger --function my-handler --source-type sns --body '{"message": "hello"}'
```

See [Triggers](./triggers.md) for details.

## Comparison

### How is this different from SAM CLI?

SAM CLI requires a `template.yaml` (CloudFormation) and uses Docker for local invocation. Lambdaform reads your existing Terraform files directly and runs most runtimes natively (faster startup, no Docker overhead). If you already use Terraform, Lambdaform means zero extra configuration.

### How is this different from LocalStack?

LocalStack emulates entire AWS services (S3, DynamoDB, SQS, etc.). Lambdaform focuses specifically on Lambda + API Gateway local development. Use both together: LocalStack for AWS service emulation, Lambdaform for fast Lambda iteration.

### How is this different from serverless-offline?

serverless-offline requires the Serverless Framework (`serverless.yml`). Lambdaform works with Terraform/OpenTofu. If your infrastructure is in Terraform, Lambdaform is the native choice.

## Limitations

### What doesn't Lambdaform support?

- **Remote Terraform modules** — only local modules are supported
- **`count`/`for_each`** — resources using these are warned about but not fully expanded
- **Complex HCL expressions** — simple variable interpolation works; deeply nested expressions may fall back to raw strings
- **AWS service emulation** — Lambdaform doesn't emulate S3, DynamoDB, etc. Use LocalStack or DynamoDB Local alongside it
- **Custom authorizer caching** — authorizer results are not cached between requests (every request triggers the authorizer)

### Is there a request size limit?

Yes, matching AWS Lambda limits:
- **Request body:** 6 MB (synchronous invocation)
- **Response body:** 6 MB
- **API Gateway payload:** 10 MB
