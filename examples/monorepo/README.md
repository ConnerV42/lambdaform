# Monorepo — Multiple API Gateways

Demonstrates a monorepo with **3 separate API Gateways** running on different ports, mixed runtimes, and shared layers.

## Architecture

- **Users API** (REST v1) — Node.js, 2 endpoints (`GET /users`, `GET /users/{userId}`)
- **Products API** (HTTP v2) — Python, 2 endpoints (`GET /products`, `GET /products/{productId}`)
- **Orders API** (REST v1) — Python, 1 endpoint (`POST /orders`)
- **Shared layer** — Utility code shared across services

## What This Tests

- Multi-gateway routing (each API gets its own port)
- Mixed v1 REST + v2 HTTP gateways in one project
- Mixed runtimes (Node.js + Python) in one project
- Lambda layers shared across functions
- Variable interpolation in function names and environment variables

## Run It

```bash
cd examples/monorepo
lambdaform start
```

Lambdaform will assign separate ports for each API Gateway:

```
Users API (REST):    http://localhost:3000
Products API (HTTP): http://localhost:3001
Orders API (REST):   http://localhost:3002
```

## Test

```bash
# Users API
curl http://localhost:3000/users
curl http://localhost:3000/users/123

# Products API
curl http://localhost:3001/products
curl http://localhost:3001/products/456

# Orders API
curl -X POST http://localhost:3002/orders -d '{"userId":"123","productId":"456"}'
```
