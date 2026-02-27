# Full Stack: Lambdaform + DynamoDB + S3

A file upload API using two Lambda functions, DynamoDB for metadata, and S3 for file storage — all running locally.

## Quick Start

```bash
# 1. Start infrastructure (DynamoDB Local + LocalStack S3)
docker compose up -d

# 2. Start Lambdaform
lambdaform start

# 3. Upload a file
curl -s -X POST http://localhost:3000/uploads \
  -d '{"filename": "hello.txt", "content": "Hello, world!"}' | jq

# 4. List uploads
curl -s http://localhost:3000/uploads | jq

# 5. Get a specific upload
curl -s http://localhost:3000/uploads/{id} | jq
```

## Services

| Service | Port | Purpose |
|---------|------|---------|
| Lambdaform | 3000 | API Gateway + Lambda runtime |
| DynamoDB Local | 8000 | Upload metadata storage |
| LocalStack (S3) | 4566 | File storage |

## Architecture

```
                    ┌─────────────────────┐
                    │    Lambdaform        │
Client ──HTTP──────►│    (port 3000)       │
                    │                      │
                    │  POST /uploads ──► upload-handler ──► S3 + DynamoDB
                    │  GET  /uploads ──► list-handler   ──► DynamoDB
                    └─────────────────────┘
```

## Notes

- Lambda functions use `DYNAMODB_URL` and `S3_URL` environment variables to connect to local services
- In production, remove these env vars — the AWS SDK will use the default endpoints
- LocalStack free tier supports S3 — no license needed
- Requires `boto3` in your Python environment (`pip install boto3`)

## Teardown

```bash
docker compose down
```
