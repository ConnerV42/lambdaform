import json
from datetime import datetime, timezone

# In-memory data store (persists across warm invocations)
items = {}
next_id = 1

# Initialize with sample data
if not items:
    items["1"] = {"id": "1", "name": "Sample Item 1", "description": "First sample item", "createdAt": datetime.now(timezone.utc).isoformat()}
    items["2"] = {"id": "2", "name": "Sample Item 2", "description": "Second sample item", "createdAt": datetime.now(timezone.utc).isoformat()}
    next_id = 3


def handler(event, context):
    print(f"Received event: {json.dumps(event, indent=2)}")

    http_method = event.get("httpMethod", "")
    path = event.get("path", "")
    path_parameters = event.get("pathParameters") or {}
    body = event.get("body")

    try:
        # Route to appropriate handler
        if path == "/items" and http_method == "GET":
            return list_items()

        if path == "/items" and http_method == "POST":
            return create_item(body)

        if path.startswith("/items/") and http_method == "GET":
            return get_item(path_parameters.get("id"))

        if path.startswith("/items/") and http_method == "PUT":
            return update_item(path_parameters.get("id"), body)

        if path.startswith("/items/") and http_method == "DELETE":
            return delete_item(path_parameters.get("id"))

        # No matching route
        return response(404, {"error": "Not Found", "path": path, "method": http_method})

    except Exception as e:
        print(f"Error: {e}")
        return response(500, {"error": "Internal Server Error", "message": str(e)})


def response(status_code, body):
    return {
        "statusCode": status_code,
        "headers": {"Content-Type": "application/json"},
        "body": json.dumps(body),
    }


def list_items():
    all_items = list(items.values())
    return response(200, {"items": all_items, "count": len(all_items)})


def create_item(body_string):
    global next_id

    if not body_string:
        return response(400, {"error": "Bad Request", "message": "Request body is required"})

    data = json.loads(body_string)

    if "name" not in data:
        return response(400, {"error": "Bad Request", "message": "name field is required"})

    item_id = str(next_id)
    next_id += 1

    item = {
        "id": item_id,
        "name": data["name"],
        "description": data.get("description", ""),
        "createdAt": datetime.now(timezone.utc).isoformat(),
    }
    items[item_id] = item
    return response(201, item)


def get_item(item_id):
    item = items.get(item_id)
    if not item:
        return response(404, {"error": "Not Found", "message": f"Item {item_id} not found"})
    return response(200, item)


def update_item(item_id, body_string):
    item = items.get(item_id)
    if not item:
        return response(404, {"error": "Not Found", "message": f"Item {item_id} not found"})

    if not body_string:
        return response(400, {"error": "Bad Request", "message": "Request body is required"})

    data = json.loads(body_string)
    updated = {
        **item,
        "name": data.get("name", item["name"]),
        "description": data.get("description", item["description"]),
        "updatedAt": datetime.now(timezone.utc).isoformat(),
    }
    items[item_id] = updated
    return response(200, updated)


def delete_item(item_id):
    item = items.get(item_id)
    if not item:
        return response(404, {"error": "Not Found", "message": f"Item {item_id} not found"})

    del items[item_id]
    return response(200, {"message": "Item deleted successfully", "id": item_id})
