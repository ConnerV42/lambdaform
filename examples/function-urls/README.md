# Lambda Function URLs Example

Demonstrates **Lambda Function URLs** — direct HTTPS endpoints for Lambda functions, no API Gateway needed.

## What's Covered

- Two functions with their own Function URLs (separate ports)
- **Node.js 20** greeting function (public, `NONE` auth)
- **Python 3.12** echo function (IAM auth — locally no auth enforced)
- CORS configuration per function URL
- Environment variables

## Usage

```bash
cd examples/function-urls
lambdaform start
```

Each function URL gets its own port:

```bash
# Greeting (port shown in startup output)
curl "http://localhost:3001/?name=Developer"

# Echo — returns full request details
curl -X POST http://localhost:3002/ \
  -H "Content-Type: application/json" \
  -d '{"test": "data"}'
```

## Key Difference from API Gateway

Function URLs give each function a direct endpoint. There's no routing, no stages, no resource paths — just one function per URL. In Lambdaform, each Function URL gets its own port.

| Feature | API Gateway | Function URL |
|---------|------------|--------------|
| Routing | Path-based, multiple functions | One function per URL |
| Auth | Authorizers, API keys | IAM or NONE |
| Cost | Per-request + data transfer | Included in Lambda pricing |
| Latency | +~10ms overhead | Direct |
