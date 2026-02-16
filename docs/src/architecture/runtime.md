# Runtime Engine

The runtime engine (`runtime.rs`) handles Lambda function invocation across all supported languages.

## Invocation Strategy

### Node.js & Python (Warm Pool)

1. A wrapper script is generated that imports the handler module
2. The wrapper listens on stdin for JSON event payloads
3. On invocation, the event is written to stdin
4. The handler response is read from stdout
5. The process stays alive for reuse (warm pool)

This achieves ~3ms warm invocation times.

### Go (Mini RIE)

Go functions use a lightweight Runtime Interface Emulator:

1. The compiled binary is spawned
2. An HTTP server acts as the Lambda Runtime API (`/2018-06-01/runtime/invocation/`)
3. The binary polls for events via the standard Lambda runtime interface
4. Responses are collected via the runtime API

This is slower (~14ms) but compatible with any `provided.al2023` binary.

## Timeout Enforcement

Each invocation is wrapped in a timeout (from Terraform's `timeout` attribute, default 3s). If exceeded:
1. The process is killed (SIGKILL)
2. A timeout error response is returned
3. The pool slot is freed

## Environment Variables

Every invocation receives:
- User-defined variables from Terraform `environment.variables`
- Config overrides from `lambdaform.yaml`
- Standard Lambda environment variables:
  - `AWS_LAMBDA_FUNCTION_NAME`
  - `AWS_LAMBDA_FUNCTION_VERSION` (`$LATEST`)
  - `_HANDLER`
  - `LAMBDA_TASK_ROOT`
  - `AWS_REGION` / `AWS_DEFAULT_REGION`

## Error Handling

Runtime errors are translated to proper Lambda error responses:
- **Handler exception** → `{"errorMessage": "...", "errorType": "Error"}` with 200 status (matching AWS behavior)
- **Process crash** → 502 Bad Gateway
- **Timeout** → 504 Gateway Timeout
