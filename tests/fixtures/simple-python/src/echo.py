"""Echo handler - returns the request body"""

import json


def handler(event, context):
    body = event.get("body")
    if body and isinstance(body, str):
        try:
            body = json.loads(body)
        except json.JSONDecodeError:
            pass

    return {
        "statusCode": 200,
        "headers": {
            "Content-Type": "application/json",
        },
        "body": json.dumps({
            "echo": body,
            "method": event.get("httpMethod", "UNKNOWN"),
        }),
    }
