# Why Local Lambda Dev with Terraform Is Broken (and How I Fixed It)

If you use Terraform to manage AWS Lambda functions, you've probably felt the pain of local development. I did too — so I built [Lambdaform](https://github.com/ConnerV42/lambdaform), a Terraform-native local Lambda emulator that just reads your `.tf` files.

No Docker. No CloudFormation. No LocalStack account. One binary, instant startup.

## The Problem

Here's the state of local Lambda development in 2026:

**LocalStack** is going full account-required (March 2026). The free tier has been shrinking for years, Docker is mandatory, and it doesn't understand your Terraform — you're deploying to a fake cloud, not running locally.

**SAM CLI** added "beta" Terraform support over two years ago. It's still beta. It requires Docker, generates CloudFormation under the hood, and the Terraform integration is brittle. A [422-point HN thread](https://news.ycombinator.com/item?id=39895517) captures the frustration.

**serverless-offline** is great — if you use the Serverless Framework. If you use Terraform, you're maintaining a duplicate configuration just for local dev.

The common thread: **none of these tools treat Terraform as a first-class citizen**. They all want you to translate your infrastructure into their format first.

## The Fix: Just Read the Terraform

Lambdaform takes a different approach. It parses your `.tf` files directly — your `aws_lambda_function`, `aws_api_gateway_rest_api`, `aws_apigatewayv2_api` resources — and spins up a local HTTP server that routes requests to your actual handler code.

```bash
cd my-terraform-project
lambdaform start
# → Server running at http://localhost:3000
```

That's it. No config file needed (though `lambdaform.yaml` exists for overrides). No Docker daemon. No account signup.

### What You Get

- **REST API Gateway (v1)** and **HTTP API Gateway (v2)** — both supported, with proper event format differences
- **Lambda authorizers** — TOKEN and REQUEST types, so your auth flow works locally
- **Hot reload** — change your handler code or `.tf` files, Lambdaform picks it up instantly
- **Warm process pool** — ~3ms warm invocations after initial cold start
- **Debugger integration** — `--inspect-brk` for Node.js, `debugpy` for Python, `delve` for Go
- **WebSocket APIs** — `$connect`/`$disconnect`/custom routes with `@connections` management
- **Lambda layers** — automatic path resolution
- **SQS/SNS trigger simulation** — test event source mappings locally
- **OpenTofu compatible** — works identically with both

### Runtimes

Node.js, Python, and Go — covering the vast majority of Lambda usage. Rust support (via `provided.al2023`) is on the roadmap.

## The Approach: Parse HCL, Not Deploy It

Lambdaform doesn't try to simulate AWS. It doesn't run CloudFormation. It reads your HCL to understand:

1. **What functions exist** — resource names, handlers, runtimes, environment variables
2. **How they're routed** — API Gateway integrations, methods, paths
3. **What connects to what** — layers, authorizers, triggers

Then it builds a local router that maps HTTP requests → Lambda invocations using your actual code. The goal is **development speed**, not AWS fidelity. When you need full fidelity, deploy to a dev account.

## Why Not Just Use Docker?

Docker adds ~2-5 seconds of cold start overhead per invocation, requires a running daemon, and consumes significant memory. On a laptop, that friction adds up fast.

Lambdaform spawns native processes — Node.js, Python, or Go — with a minimal Lambda Runtime Interface shim. Cold starts are ~100ms. Warm invocations are ~3ms. The process pool keeps workers alive between requests.

For a tight dev loop (change code → test → iterate), this speed difference compounds dramatically over a day.

## Quick Start

```bash
# Homebrew (macOS/Linux)
brew tap ConnerV42/lambdaform
brew install lambdaform

# Cargo
cargo install lambdaform

# npm (for the npx crowd)
npx lambdaform start
```

Validate your setup:

```bash
lambdaform validate
# 🔍 Found 3 function(s), 1 gateway(s), 3 route(s)
# ✅ Validation passed!
```

Start the server:

```bash
lambdaform start
# 📦 Lambda Functions:
#    • hello-world (Nodejs20) → index.handler
#    • get-user (Nodejs20) → users.handler  
# 🔥 Server running at http://localhost:3000
```

## What's Next

Lambdaform is at v0.5.0. The core is solid — 62 tests passing, cross-platform CI, proper error handling, graceful shutdown. The roadmap includes:

- **Terraform variable resolution** from `.tfvars`
- **Module support** for local Terraform modules  
- **`lambdaform init`** for guided setup
- **Request replay** for debugging
- **Rust runtime** support
- **VS Code extension**

## Open Source, No Tricks

MIT license. No paid tier. No feature gating. No telemetry.

If local Lambda development with Terraform has frustrated you, give it a try:

**GitHub:** [github.com/ConnerV42/lambdaform](https://github.com/ConnerV42/lambdaform)

Feedback, issues, and PRs welcome.
