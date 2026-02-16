# CLI Reference

## Global Options

| Flag | Description |
|------|-------------|
| `--help`, `-h` | Show help |
| `--version`, `-V` | Show version |

## `lambdaform start`

Start the local development server.

```bash
lambdaform start [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--port <PORT>` | `3000` | Server port |
| `--dir <DIR>` | `.` | Terraform directory to scan |
| `--verbose` | off | Enable detailed request logging |
| `--debug` | off | Enable Node.js debugger (Inspector protocol) |
| `--debug-python` | off | Enable Python debugger (debugpy) |
| `--debug-port <PORT>` | `9229` | Node.js debug port |
| `--debug-python-port <PORT>` | `5678` | Python debug port |
| `--no-watch` | off | Disable hot reload |
| `--json-log` | off | Structured JSON log output |
| `--var-file <FILE>` | — | Load `.tfvars` file (repeatable) |
| `--record` | off | Record requests to history file |
| `--tui` | off | Enable terminal UI dashboard |

## `lambdaform invoke`

Invoke a Lambda function directly (bypassing API Gateway).

```bash
lambdaform invoke <FUNCTION_NAME> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--event <JSON>` | JSON event payload (inline) |
| `--event-file <FILE>` | JSON event payload (from file) |
| `--dir <DIR>` | Terraform directory |
| `--var-file <FILE>` | Load `.tfvars` file (repeatable) |

## `lambdaform config`

Display parsed Lambda and API Gateway configuration.

```bash
lambdaform config [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--json` | Output as JSON |
| `--dir <DIR>` | Terraform directory |
| `--var-file <FILE>` | Load `.tfvars` file (repeatable) |

## `lambdaform validate`

Validate Terraform files and report discovered resources.

```bash
lambdaform validate [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--dir <DIR>` | Terraform directory |
| `--var-file <FILE>` | Load `.tfvars` file (repeatable) |

## `lambdaform trigger`

Send simulated SQS or SNS events to a Lambda function.

```bash
lambdaform trigger <TYPE> <RESOURCE_NAME> <BODY> [OPTIONS]
```

- `TYPE`: `sqs` or `sns`
- `RESOURCE_NAME`: Terraform resource name of the queue/topic
- `BODY`: JSON message body

| Option | Description |
|--------|-------------|
| `--batch <N>` | Number of records in batch (default: 1) |
| `--function <NAME>` | Target function (if multiple subscribers) |
| `--dir <DIR>` | Terraform directory |

## `lambdaform stepfunctions` / `sfn`

Visualize Step Functions state machines as ASCII diagrams.

```bash
lambdaform stepfunctions [OPTIONS]
lambdaform sfn [OPTIONS]           # alias
```

| Option | Description |
|--------|-------------|
| `--name <NAME>` | Show specific state machine |
| `--json` | Output as JSON |
| `--dir <DIR>` | Terraform directory |

## `lambdaform replay`

Replay recorded requests from history.

```bash
lambdaform replay [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--list` | List recorded requests |
| `--id <ID>` | Replay specific request by index |
| `--last <N>` | Replay last N requests |
| `--all` | Replay all recorded requests |
| `--filter <PATH>` | Filter by URL path prefix |
| `--filter-method <METHOD>` | Filter by HTTP method |
| `--port <PORT>` | Target server port (default: 3000) |
| `--dir <DIR>` | Terraform directory |

## `lambdaform init`

Generate a `lambdaform.yaml` config file with guided setup.

```bash
lambdaform init [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--dir <DIR>` | Target directory |
| `--yes`, `-y` | Accept all defaults (non-interactive) |

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Error (parse failure, runtime error, etc.) |
