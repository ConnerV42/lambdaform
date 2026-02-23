# CRUD API — Python 3.12

A REST API with GET/POST/PUT/DELETE operations using Python 3.12 and API Gateway v1. Mirrors the Node.js version for parity testing.

## Quick Start

```bash
lambdaform start
# API available at http://localhost:3000

# List items
curl http://localhost:3000/items

# Create item
curl -X POST http://localhost:3000/items \
  -H 'Content-Type: application/json' \
  -d '{"name": "New Item", "description": "A test item"}'

# Get item
curl http://localhost:3000/items/1

# Update item
curl -X PUT http://localhost:3000/items/1 \
  -H 'Content-Type: application/json' \
  -d '{"name": "Updated Item"}'

# Delete item
curl -X DELETE http://localhost:3000/items/2
```

## What This Tests

- Python 3.12 runtime
- API Gateway v1 (REST API) with AWS_PROXY integration
- Path parameters (`{id}`)
- Request body parsing
- Multiple HTTP methods on same resource
- Error handling (400, 404, 500)
- In-memory state across warm invocations
