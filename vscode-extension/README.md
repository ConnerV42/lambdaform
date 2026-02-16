# Lambdaform for VS Code

Terraform-native local Lambda development, right in your editor.

## Features

- **Function Explorer** — See all Lambda functions defined in your Terraform files
- **One-Click Invoke** — Invoke any function with a click, with or without a custom payload
- **Live Log Viewer** — Watch invocations in real-time with status, duration, and filtering
- **Server Control** — Start/stop the Lambdaform server from the activity bar
- **Auto-Refresh** — Function list updates when `.tf` files change
- **Status Bar** — Always know if your server is running

## Getting Started

1. Install [Lambdaform](https://github.com/ConnerV42/lambdaform) CLI
2. Open a workspace containing Terraform files with Lambda functions
3. Click the Lambdaform icon in the activity bar
4. Click ▶ to start the server
5. Click ▶ on any function to invoke it

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `lambdaform.binaryPath` | `lambdaform` | Path to the lambdaform binary |
| `lambdaform.port` | `3000` | Server port |
| `lambdaform.terraformDir` | `.` | Terraform directory (relative to workspace) |
| `lambdaform.autoStart` | `false` | Auto-start server on workspace open |
| `lambdaform.verbose` | `false` | Verbose logging |
| `lambdaform.jsonLog` | `true` | JSON structured logs (for log parsing) |
| `lambdaform.varFiles` | `[]` | Additional `.tfvars` files |

## Commands

- `Lambdaform: Start Server` — Start the local dev server
- `Lambdaform: Stop Server` — Stop the server
- `Lambdaform: Refresh Functions` — Reload the function list
- `Lambdaform: Invoke Function` — Invoke with empty payload
- `Lambdaform: Invoke Function with Payload` — Invoke with custom JSON
- `Lambdaform: View Function Logs` — Filter logs to a specific function
- `Lambdaform: Clear Logs` — Clear the log viewer
- `Lambdaform: Open Configuration` — Open `lambdaform.yaml`
