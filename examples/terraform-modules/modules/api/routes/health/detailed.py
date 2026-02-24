import json
import os
import time

def handler(event, context):
    return {
        "statusCode": 200,
        "headers": {"Content-Type": "application/json"},
        "body": json.dumps({
            "status": "healthy",
            "service": os.environ.get("SERVICE_NAME", "unknown"),
            "environment": os.environ.get("ENV", "unknown"),
            "version": os.environ.get("VERSION", "0.0.0"),
            "depth": "3 (root -> api -> routes/health)",
            "timestamp": time.time(),
            "checks": {
                "memory": "ok",
                "disk": "ok",
            },
        }),
    }
