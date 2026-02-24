"""Orders service — Python handlers"""

import json
import os
import uuid


def create_order(event, context):
    body = json.loads(event.get("body", "{}")) if event.get("body") else {}
    order_id = str(uuid.uuid4())[:8]

    return {
        "statusCode": 201,
        "headers": {"Content-Type": "application/json"},
        "body": json.dumps({
            "orderId": order_id,
            "userId": body.get("userId", "anonymous"),
            "items": body.get("items", []),
            "status": "created",
            "tables": {
                "users": os.environ.get("USERS_TABLE", "unknown"),
                "products": os.environ.get("PRODUCTS_TABLE", "unknown"),
                "orders": os.environ.get("ORDERS_TABLE", "unknown"),
            },
        }),
    }
