# Introduction

**Lambdaform** is a Terraform-native local development server for AWS Lambda and API Gateway. It parses your `.tf` files directly — no CloudFormation, no Docker, no LocalStack account required.

## The Problem

If you manage Lambda infrastructure with Terraform, local development is painful:

- **LocalStack** requires Docker and CloudFormation translation. The free tier is limited; many features need Pro.
- **SAM CLI** is CloudFormation-native. Terraform support is experimental and fragile.
- **serverless-offline** requires a separate `serverless.yml` that duplicates your Terraform config.

All of these force you to maintain a parallel configuration for local development.

## The Solution

Lambdaform reads your Terraform directly. Your `.tf` files are the single source of truth — for both deployment and local dev.

```bash
cd my-terraform-project
lambdaform start
# → Server running at http://localhost:3000
```

It discovers your `aws_lambda_function`, `aws_api_gateway_*`, and `aws_apigatewayv2_*` resources, wires up routes, and starts a local HTTP server. Code changes trigger instant hot reload.

## Key Features

- **Zero configuration** — works out of the box with standard Terraform Lambda projects
- **All major runtimes** — Node.js, Python, Go, Rust, and Java (Docker)
- **Both API Gateway versions** — REST API (v1) and HTTP API (v2)
- **WebSocket APIs** — full `$connect`/`$disconnect`/custom route support
- **Lambda authorizers** — TOKEN and REQUEST types execute before your handler
- **Warm process pool** — ~3ms warm invocations, 97% faster than cold starts
- **Debugger integration** — attach Node.js Inspector or Python debugpy
- **Request history** — record and replay requests for testing
- **Terminal UI** — optional live dashboard with color-coded request log
- **Terraform modules** — resolves local module sources
- **OpenTofu compatible** — works identically with `.tf` files from either tool

## Who Is This For?

- Teams using **Terraform** (or OpenTofu) for Lambda infrastructure
- Developers who want **fast local iteration** without Docker overhead
- Projects with **multiple API Gateways** that need isolated local ports
- Anyone tired of maintaining **duplicate configuration** for local dev
