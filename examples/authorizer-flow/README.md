# Authorizer Flow Example

Demonstrates API Gateway v1 with a Lambda TOKEN authorizer protecting endpoints.

## Architecture

- **Token Authorizer** — validates `Authorization: Bearer <token>` header
- **Public endpoint** — `GET /public` (no auth required)
- **Protected endpoints** — `GET /protected`, `POST /protected` (auth required)

## Running

```bash
cd examples/authorizer-flow
lambdaform start
```

## Testing

```bash
# Public endpoint (no auth needed)
curl http://localhost:3000/public

# Protected endpoint WITHOUT token → 401
curl http://localhost:3000/protected

# Protected endpoint WITH valid token → 200
curl -H "Authorization: Bearer super-secret-token-123" http://localhost:3000/protected

# Protected POST with token
curl -X POST -H "Authorization: Bearer super-secret-token-123" http://localhost:3000/protected

# Invalid token → 401
curl -H "Authorization: Bearer wrong-token" http://localhost:3000/protected
```

## Expected Behavior

| Request | Expected |
|---------|----------|
| `GET /public` | 200 — public content |
| `GET /protected` (no header) | 401 — Unauthorized |
| `GET /protected` (valid token) | 200 — protected content |
| `GET /protected` (bad token) | 401 — Unauthorized |
| `POST /protected` (valid token) | 200 — protected content |
