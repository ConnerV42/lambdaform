# Rust Lambda Example

A REST API built with Rust using `lambda_http`, demonstrating Lambdaform's custom runtime support (`provided.al2023`).

## Prerequisites

- Rust toolchain (cargo) installed
- Lambdaform installed

## Structure

```
rust-lambda/
├── infra/main.tf        # API Gateway v2 + Lambda (provided.al2023)
├── src/
│   ├── Cargo.toml       # Rust dependencies
│   └── main.rs          # Handler using lambda_http
└── README.md
```

## Run locally

```bash
cd infra
lambdaform start
```

Lambdaform auto-detects the Cargo.toml, builds the Rust binary, and starts a mini Runtime Interface Emulator.

## Test

```bash
# GET root
curl http://localhost:3001/

# GET item by ID  
curl http://localhost:3001/item-42

# POST create
curl -X POST http://localhost:3001/ -d '{"name":"test"}'
```

## Features demonstrated

- Custom runtime (provided.al2023) with auto-build via cargo
- Smart rebuild (only when source changes)
- API Gateway v2 (HTTP API) event format
- Path parameters
- JSON request/response handling
