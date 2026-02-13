"""Simple Python Lambda handler for testing Lambdaform"""

import json
import os
from datetime import datetime, timezone


def handler(event, context):
    greeting = os.environ.get("GREETING", "Hello from Python!")
    env = os.environ.get("ENV", "unknown")

    params = event.get("queryStringParameters") or {}
    name = params.get("name", "stranger")

    return {
        "statusCode": 200,
        "headers": {
            "Content-Type": "application/json",
        },
        "body": json.dumps({
            "message": f"{greeting} Welcome, {name}!",
            "environment": env,
            "runtime": "python",
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "requestId": context.get("awsRequestId", "unknown") if isinstance(context, dict) else getattr(context, "aws_request_id", "unknown"),
        }),
    }
