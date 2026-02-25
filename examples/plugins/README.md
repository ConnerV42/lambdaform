# Plugins — S3 Local Emulator

Demonstrates Lambdaform's plugin architecture with a local S3 bucket emulator.

## How It Works

The plugin (`s3-local.py`) is a simple Python script that implements the Lambdaform plugin protocol:

1. **describe** — Reports capabilities (handles `aws_s3_bucket` resources)
2. **on_resource** — Creates local directories for each S3 bucket and injects `S3_ENDPOINT` env var

## Configuration

Add to your `lambdaform.yaml`:

```yaml
plugins:
  - name: s3-local
    path: ./examples/plugins/s3-local.py
    config:
      data_dir: /tmp/lambdaform-s3
```

## What This Tests

- Plugin discovery and lifecycle (describe → on_resource)
- Side effects (environment variable injection, logging)
- Plugin configuration passthrough
- External process communication (JSON over stdin/stdout)

## Plugin Protocol

Plugins communicate via JSON over stdin/stdout:

```json
// Request
{"kind": "describe", "config": {"data_dir": "/tmp/lambdaform-s3"}}

// Response
{"ok": true, "capabilities": {"version": "0.1.0", "resource_types": ["aws_s3_bucket"]}}
```

See the [Plugin Architecture docs](https://connerv42.github.io/lambdaform/advanced/plugins.html) for the full protocol specification.
