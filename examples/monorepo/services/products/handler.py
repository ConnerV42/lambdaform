"""Products service — Python handlers"""

import json
import os


def list_products(event, context):
    products = [
        {"id": "p1", "name": "Widget", "price": 9.99},
        {"id": "p2", "name": "Gadget", "price": 24.99},
        {"id": "p3", "name": "Doohickey", "price": 14.99},
    ]
    return {
        "statusCode": 200,
        "headers": {"Content-Type": "application/json"},
        "body": json.dumps({
            "products": products,
            "count": len(products),
            "table": os.environ.get("TABLE_NAME", "unknown"),
        }),
    }


def get_product(event, context):
    product_id = event.get("pathParameters", {}).get("productId", "unknown")
    return {
        "statusCode": 200,
        "headers": {"Content-Type": "application/json"},
        "body": json.dumps({
            "id": product_id,
            "name": "Widget",
            "price": 9.99,
            "table": os.environ.get("TABLE_NAME", "unknown"),
        }),
    }
