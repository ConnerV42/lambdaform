# CRUD API - Node.js

A simple REST API demonstrating full CRUD (Create, Read, Update, Delete) operations using:
- **Runtime:** Node.js 20
- **API Gateway:** REST API (v1)
- **Data Store:** In-memory Map (persists across warm invocations)

## API Endpoints

### GET /items
List all items.

```bash
curl http://localhost:3000/items
```

**Response (200):**
```json
{
  "items": [
    { "id": "1", "name": "Sample Item 1", "description": "...", "createdAt": "..." }
  ],
  "count": 2
}
```

### POST /items
Create a new item.

```bash
curl -X POST http://localhost:3000/items \
  -H "Content-Type: application/json" \
  -d '{"name": "New Item", "description": "My new item"}'
```

**Response (201):**
```json
{
  "id": "3",
  "name": "New Item",
  "description": "My new item",
  "createdAt": "2026-02-23T23:23:00.000Z"
}
```

### GET /items/{id}
Get a specific item by ID.

```bash
curl http://localhost:3000/items/1
```

**Response (200):**
```json
{
  "id": "1",
  "name": "Sample Item 1",
  "description": "First sample item",
  "createdAt": "..."
}
```

**Response (404):**
```json
{
  "error": "Not Found",
  "message": "Item 999 not found"
}
```

### PUT /items/{id}
Update an existing item.

```bash
curl -X PUT http://localhost:3000/items/1 \
  -H "Content-Type: application/json" \
  -d '{"name": "Updated Item", "description": "Updated description"}'
```

**Response (200):**
```json
{
  "id": "1",
  "name": "Updated Item",
  "description": "Updated description",
  "createdAt": "...",
  "updatedAt": "2026-02-23T23:25:00.000Z"
}
```

### DELETE /items/{id}
Delete an item.

```bash
curl -X DELETE http://localhost:3000/items/1
```

**Response (200):**
```json
{
  "message": "Item deleted successfully",
  "id": "1"
}
```

## Running with Lambdaform

1. **Start the local server:**
   ```bash
   lambdaform start
   ```

2. **Test the endpoints:**
   ```bash
   # List all items
   curl http://localhost:3000/items

   # Create an item
   curl -X POST http://localhost:3000/items \
     -H "Content-Type: application/json" \
     -d '{"name": "Test Item", "description": "Testing CRUD"}'

   # Get specific item (use ID from create response)
   curl http://localhost:3000/items/3

   # Update item
   curl -X PUT http://localhost:3000/items/3 \
     -H "Content-Type: application/json" \
     -d '{"name": "Updated Test", "description": "Updated via PUT"}'

   # Delete item
   curl -X DELETE http://localhost:3000/items/3

   # Verify deletion
   curl http://localhost:3000/items/3  # Should return 404
   ```

## Features Demonstrated

- ✅ API Gateway REST API (v1)
- ✅ Multiple HTTP methods (GET, POST, PUT, DELETE)
- ✅ Path parameters (`{id}`)
- ✅ Request body parsing
- ✅ Proper HTTP status codes (200, 201, 400, 404, 500)
- ✅ JSON responses
- ✅ Error handling
- ✅ In-memory state (persists across warm invocations)

## Architecture

```
GET    /items         → list all items
POST   /items         → create new item
GET    /items/{id}    → get specific item
PUT    /items/{id}    → update item
DELETE /items/{id}    → delete item
```

All routes are handled by a single Lambda function (`crud-api-handler`) that routes internally based on the HTTP method and path.
