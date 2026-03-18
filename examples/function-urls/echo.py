"""Echo function — returns the full request details.

Useful for debugging what Lambda Function URLs receive.
"""

import json


def handler(event, context):
    return {
        "statusCode": 200,
        "headers": {
            "Content-Type": "application/json",
        },
        "body": json.dumps(
            {
                "echo": {
                    "method": event.get("requestContext", {})
                    .get("http", {})
                    .get("method", "UNKNOWN"),
                    "path": event.get("rawPath", "/"),
                    "queryString": event.get("rawQueryString", ""),
                    "headers": event.get("headers", {}),
                    "body": event.get("body"),
                    "isBase64Encoded": event.get("isBase64Encoded", False),
                },
                "functionUrl": True,
            },
            indent=2,
        ),
    }
