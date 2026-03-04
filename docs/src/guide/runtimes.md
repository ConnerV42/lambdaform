# Lambda Runtimes

Lambdaform supports multiple runtime families with different invocation strategies.

## Node.js

**Supported:** `nodejs18.x`, `nodejs20.x`, `nodejs22.x`

Invocation: Lambdaform spawns a Node.js process that loads your handler module and calls the exported function. Warm process pooling keeps a pool of ready processes for ~3ms invocations.

```hcl
resource "aws_lambda_function" "api" {
  runtime  = "nodejs20.x"
  handler  = "index.handler"    # file: index.js, export: handler
}
```

Handler format: `<file>.<export>` (e.g., `src/handler.processEvent`)

### Environment Variables

All Terraform-defined environment variables are passed to the Node.js process:

```hcl
environment {
  variables = {
    TABLE_NAME = "users"
    STAGE      = "local"
  }
}
```

Lambdaform also sets:
- `AWS_LAMBDA_FUNCTION_NAME`
- `AWS_LAMBDA_FUNCTION_VERSION` (`$LATEST`)
- `_HANDLER`
- `LAMBDA_TASK_ROOT`

## Python

**Supported:** `python3.10`, `python3.11`, `python3.12`, `python3.13`

Invocation: Same warm pool strategy as Node.js. Lambdaform spawns Python processes that import your handler module.

```hcl
resource "aws_lambda_function" "processor" {
  runtime  = "python3.12"
  handler  = "app.handler"    # file: app.py, function: handler
}
```

Handler format: `<module>.<function>` (e.g., `handlers.users.create_user`)

### Python Path

Lambdaform sets `PYTHONPATH` to include the function's source directory, so relative imports work as expected.

## Go

**Supported:** `go1.x`, `provided.al2`, `provided.al2023`

Invocation: Go functions use a mini Runtime Interface Emulator (RIE). The compiled binary must exist at the handler path.

```hcl
resource "aws_lambda_function" "go_fn" {
  runtime  = "provided.al2023"
  handler  = "bootstrap"      # compiled binary name
}
```

> **Note:** You must compile the Go binary before running Lambdaform. Hot reload detects binary changes and restarts.

### Build Workflow

```bash
# Build your Go Lambda
GOOS=linux GOARCH=amd64 go build -o bootstrap main.go

# Start Lambdaform
lambdaform start
```

## Rust

**Supported:** `provided.al2023` (with Cargo project detection)

Lambdaform auto-detects Rust Lambda projects by looking for `Cargo.toml` in the source directory. When detected, it runs `cargo build` automatically and uses the compiled binary.

```hcl
resource "aws_lambda_function" "rust_fn" {
  runtime  = "provided.al2023"
  handler  = "bootstrap"
}
```

Hot reload watches for `.rs` file changes and triggers a rebuild automatically.

## Java (Docker)

**Supported:** `java8.al2`, `java11`, `java17`, `java21`

Java runtimes require Docker. Lambdaform pulls the official AWS Lambda base images and runs your function inside them for full JVM environment parity.

```hcl
resource "aws_lambda_function" "java_fn" {
  runtime  = "java21"
  handler  = "com.example.Handler::handleRequest"
}
```

> **Note:** Docker must be running. On first use, Lambdaform pulls the AWS Lambda base image (~500MB). Subsequent starts use the cached image.

See [Java Runtime Details](../runtimes/java.md) for build setup and troubleshooting.

## Performance

| Runtime | Cold Start | Warm Invocation |
|---------|-----------|-----------------|
| Node.js | ~100ms | ~3ms |
| Python | ~120ms | ~3ms |
| Go | ~14ms | ~14ms (no pool) |
| Rust | ~10ms | ~10ms (no pool) |
| Java | ~3-8s | ~50ms (Docker) |

Warm process pooling is enabled by default for Node.js and Python. It's disabled automatically when debug mode is active.

## Timeout Enforcement

Lambdaform enforces the `timeout` attribute from your Terraform (default: 3 seconds, max: 900). If a function exceeds its timeout, the process is killed and a timeout error is returned.
