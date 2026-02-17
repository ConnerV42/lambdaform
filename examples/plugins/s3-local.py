#!/usr/bin/env python3
"""
Example Lambdaform plugin: Local S3 bucket emulator.

This plugin demonstrates the Lambdaform plugin protocol by:
- Detecting aws_s3_bucket resources during Terraform parsing
- Creating local directories to simulate buckets
- Injecting S3_ENDPOINT env vars into Lambda functions

Usage in lambdaform.yaml:
  plugins:
    - name: s3-local
      path: ./examples/plugins/s3-local.py
      config:
        data_dir: /tmp/lambdaform-s3
"""

import json
import os
import sys


def handle_describe(config):
    """Return plugin capabilities."""
    return {
        "ok": True,
        "capabilities": {
            "version": "0.1.0",
            "resource_types": ["aws_s3_bucket", "aws_s3_object"],
            "intercept_requests": False,
            "intercept_responses": False,
            "description": "Local S3 bucket emulator (creates directories for buckets)",
        },
    }


def handle_on_resource(request):
    """Handle Terraform resource discovery."""
    resource_type = request["resource_type"]
    resource_name = request["resource_name"]
    attributes = request["attributes"]
    config = request["config"]
    data_dir = config.get("data_dir", "/tmp/lambdaform-s3")

    if resource_type == "aws_s3_bucket":
        bucket_name = attributes.get("bucket", resource_name)
        bucket_path = os.path.join(data_dir, bucket_name)
        os.makedirs(bucket_path, exist_ok=True)

        return {
            "ok": True,
            "side_effects": [
                {
                    "kind": "env_var",
                    "functions": [],
                    "key": "S3_ENDPOINT",
                    "value": f"file://{data_dir}",
                },
                {
                    "kind": "log",
                    "level": "info",
                    "message": f"Created local bucket: {bucket_path}",
                },
            ],
        }

    return {"ok": True}


def main():
    request = json.loads(sys.stdin.read())
    kind = request["kind"]

    if kind == "describe":
        response = handle_describe(request.get("config", {}))
    elif kind == "on_resource":
        response = handle_on_resource(request)
    else:
        response = {"ok": True}

    print(json.dumps(response))


if __name__ == "__main__":
    main()
