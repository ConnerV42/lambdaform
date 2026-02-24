# Go Lambda Example

A simple REST API built with Go, demonstrating Lambdaform's Go runtime support.

## Prerequisites

- Go 1.21+ installed and in PATH
- Lambdaform installed

## Structure

```
go-lambda/
├── infra/main.tf      # API Gateway v2 + Lambda (go1.x)
├── src/
│   ├── main.go        # Handler with GET/POST routes
│   └── go.mod         # Go module definition
└── README.md
```

## Run locally

```bash
cd infra
lambdaform start
```

Lambdaform will auto-build the Go binary and start a local server.

## Test

```bash
# GET root
curl http://localhost:3001/

# GET item by ID
curl http://localhost:3001/abc-123

# POST create
curl -X POST http://localhost:3001/ -d '{"name":"test"}'
```

## Features demonstrated

- Go 1.x runtime with auto-build
- API Gateway v2 (HTTP API) event format
- Path parameters
- JSON request/response
- Environment variables
