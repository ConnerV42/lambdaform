# Multi-Function API with Shared Layer

Demonstrates multiple Lambda functions sharing code via Lambda Layers and using environment variables.

## Architecture

- **3 Lambda Functions:**
  - `user-service` — manages user data
  - `order-service` — manages orders
  - `notification-service` — sends notifications

- **1 Shared Layer:**
  - `common-utils` — shared utilities (response formatting, logging, validation)

- **Environment Variables:**
  - Global: `API_VERSION`, `SERVICE_NAME`, `ENVIRONMENT`, `LOG_LEVEL`
  - Service-specific: `MAX_ORDERS`, `EMAIL_ENABLED`, `SMS_ENABLED`

## API Endpoints

### GET /users
List all users with metadata.

```bash
curl http://localhost:3000/users
```

**Response (200):**
```json
{
  "success": true,
  "message": "Users retrieved successfully",
  "data": {
    "users": [
      { "id": "1", "name": "Alice Johnson", "email": "alice@example.com", "role": "admin" }
    ],
    "count": 3
  },
  "meta": {
    "service": "user-service",
    "version": "v1",
    "environment": "development",
    "timestamp": "2026-02-23T23:40:00.000Z"
  }
}
```

### GET /orders
List all orders (limited by `MAX_ORDERS` env var).

```bash
curl http://localhost:3000/orders
```

**Response (200):**
```json
{
  "success": true,
  "message": "Orders retrieved successfully",
  "data": {
    "orders": [...],
    "count": 3,
    "limit": 100
  },
  "meta": {
    "service": "order-service",
    "version": "v1",
    "environment": "development",
    "timestamp": "2026-02-23T23:40:00.000Z"
  }
}
```

### POST /notifications
Send a notification via enabled channels.

```bash
curl -X POST http://localhost:3000/notifications \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Your order has shipped!",
    "recipient": "customer@example.com",
    "type": "email"
  }'
```

**Response (200):**
```json
{
  "success": true,
  "message": "Notification sent successfully",
  "data": {
    "notification": {
      "id": 1,
      "message": "Your order has shipped!",
      "recipient": "customer@example.com",
      "type": "email",
      "timestamp": "...",
      "channels": {
        "email": true,
        "sms": false
      }
    },
    "sentVia": ["email"]
  },
  "meta": {
    "service": "notification-service",
    "timestamp": "..."
  }
}
```

## Running with Lambdaform

```bash
# Start the local server
lambdaform start

# Test all three services
curl http://localhost:3000/users
curl http://localhost:3000/orders
curl -X POST http://localhost:3000/notifications \
  -H "Content-Type: application/json" \
  -d '{"message": "Test notification"}'
```

## Features Demonstrated

- ✅ Multiple Lambda functions
- ✅ Lambda Layers for shared code
- ✅ Environment variables (globals + service-specific)
- ✅ Terraform locals interpolation
- ✅ Shared utilities across functions
- ✅ Structured logging
- ✅ Environment variable validation
- ✅ Different log levels per service

## Layer Structure

```
nodejs/
└── node_modules/
    └── common/
        └── index.js  ← Shared utilities
```

Functions import from the layer:
```javascript
// Require by module name (Lambdaform sets NODE_PATH)
const { successResponse, errorResponse, logInfo } = require('common');

// NOT: require('/opt/nodejs/node_modules/common')
// The /opt path is AWS Lambda convention, but locally you require by name
```

## Environment Variables

Each function has access to:
- `API_VERSION` — from locals (v1)
- `SERVICE_NAME` — unique per function
- `ENVIRONMENT` — from locals (development)
- `LOG_LEVEL` — info/debug, varies per service
- Service-specific vars (e.g., `MAX_ORDERS`, `EMAIL_ENABLED`)

The shared layer's utilities automatically include metadata from these variables in responses.
