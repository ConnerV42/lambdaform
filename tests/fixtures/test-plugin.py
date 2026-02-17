#!/usr/bin/env python3
"""Test plugin for integration tests."""
import json, sys

request = json.loads(sys.stdin.read())

if request["kind"] == "describe":
    print(json.dumps({
        "ok": True,
        "capabilities": {
            "version": "0.1.0",
            "resource_types": ["aws_s3_bucket"],
            "intercept_requests": True,
            "intercept_responses": True,
            "description": "Test plugin for integration tests"
        }
    }))
elif request["kind"] == "on_resource":
    print(json.dumps({
        "ok": True,
        "side_effects": [
            {"kind": "env_var", "functions": [], "key": "TEST_PLUGIN_KEY", "value": "test_value"},
            {"kind": "log", "level": "info", "message": "Test plugin saw resource: " + request["resource_name"]}
        ]
    }))
elif request["kind"] == "on_request":
    event = request["event"]
    event["x-plugin-injected"] = "true"
    print(json.dumps({"ok": True, "event": event}))
elif request["kind"] == "on_response":
    print(json.dumps({"ok": True}))
else:
    print(json.dumps({"ok": True}))
