import json
import os

def handler(event, context):
    return {
        "statusCode": 200,
        "headers": {"Content-Type": "application/json"},
        "body": json.dumps({
            "status": "healthy",
            "service": os.environ.get("SERVICE_NAME", "unknown"),
            "environment": os.environ.get("ENV", "unknown"),
        }),
    }
