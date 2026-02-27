# DynamoDB Local + Lambdaform

A CRUD API backed by DynamoDB Local — the most common Lambda + DynamoDB development pattern.

## Quick Start

```bash
# 1. Start DynamoDB Local (creates the table automatically)
docker compose up -d

# 2. Install dependencies
npm install @aws-sdk/client-dynamodb

# 3. Start Lambdaform
lambdaform start

# 4. Test it
curl -s http://localhost:3000/items | jq                          # List items
curl -s -X POST http://localhost:3000/items -d '{"name":"test"}' | jq   # Create
curl -s http://localhost:3000/items/{id} | jq                     # Get by ID
curl -s -X DELETE http://localhost:3000/items/{id} | jq           # Delete
```

## How It Works

- **DynamoDB Local** runs in Docker on port 8000
- **Lambdaform** reads `main.tf`, discovers the Lambda function and API Gateway routes
- The Lambda function connects to DynamoDB Local via `AWS_ENDPOINT_URL`
- No AWS credentials or cloud resources needed

## Architecture

```
Client → Lambdaform (port 3000) → Lambda function → DynamoDB Local (port 8000)
```

## Teardown

```bash
docker compose down    # Stop DynamoDB
# Ctrl+C Lambdaform
```
