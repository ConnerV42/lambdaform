# Lambdaform Examples

Each example is a complete, runnable project. Start any of them with `lambdaform start` from its directory.

## Core

| Example | Description | Runtime | API Type |
|---------|-------------|---------|----------|
| [crud-api-node](crud-api-node/) | REST API with full CRUD | Node.js 20 | REST v1 |
| [crud-api-python](crud-api-python/) | Same CRUD API in Python | Python 3.12 | REST v1 |
| [api-gateway-v2](api-gateway-v2/) | HTTP API (v2 event format) | Node.js 20 | HTTP v2 |
| [multi-function](multi-function/) | Multiple functions + shared layers | Node.js + Python | REST v1 |

## Advanced

| Example | Description | Features |
|---------|-------------|----------|
| [websocket-chat](websocket-chat/) | WebSocket chat server | $connect/$disconnect, @connections |
| [sqs-processor](sqs-processor/) | SQS-triggered processing | Event source mappings, batch/DLQ |
| [sns-fanout](sns-fanout/) | SNS fan-out to multiple consumers | Topic subscriptions, multi-runtime |
| [step-functions](step-functions/) | State machine workflow | Choice/Parallel/Wait, visualization |
| [authorizer-flow](authorizer-flow/) | Lambda authorizer | TOKEN auth, public/protected routes |
| [monorepo](monorepo/) | Multiple API Gateways | Multi-gateway, mixed runtimes, layers |

## Runtimes

| Example | Description | Runtime |
|---------|-------------|---------|
| [go-lambda](go-lambda/) | Go REST API | Go (provided.al2023) |
| [rust-lambda](rust-lambda/) | Rust REST API | Rust (provided.al2023) |

## Docker Compose

| Example | Description | Services |
|---------|-------------|----------|
| [docker-compose-dynamodb](docker-compose-dynamodb/) | CRUD API + DynamoDB Local | DynamoDB Local |
| [docker-compose-fullstack](docker-compose-fullstack/) | File upload API + DynamoDB + S3 | DynamoDB Local, LocalStack S3 |

## Edge Cases

| Example | Description |
|---------|-------------|
| [terraform-modules](terraform-modules/) | 3 levels of nested modules |
| [large-payload](large-payload/) | Binary payloads, base64, size limits |
| [plugins](plugins/) | Plugin architecture (S3 emulator) |
