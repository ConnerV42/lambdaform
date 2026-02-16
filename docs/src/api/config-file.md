# Configuration File Reference

Complete reference for `lambdaform.yaml`.

## Schema

```yaml
# Required
version: 1

# Optional: per-function overrides (key = Terraform resource name)
functions:
  <resource_name>:
    source: <string>           # Handler source directory
    handler: <string>          # Handler path (file.export)
    timeout: <integer>         # Timeout in seconds
    memory: <integer>          # Memory in MB (informational)
    environment:               # Additional env vars
      <KEY>: <VALUE>

# Optional: global environment variables
environment:
  <KEY>: <VALUE>

# Optional: default gateway port
gateway:
  port: <integer>              # Default: 3000

# Optional: per-gateway port overrides
gateways:
  <resource_name>:
    port: <integer>

# Optional: CORS configuration
cors:
  allow_origins:               # List of allowed origins
    - <string>
  allow_methods:               # List of allowed HTTP methods
    - <string>
  allow_headers:               # List of allowed headers
    - <string>

# Optional: file watcher
watch:
  enabled: <boolean>           # Default: true
  ignore:                      # Directories to ignore
    - <string>

# Optional: debugger
debug:
  enabled: <boolean>           # Default: false
  port: <integer>              # Node.js inspector port (default: 9229)
  python: <boolean>            # Enable Python debugpy (default: false)
  python_port: <integer>       # debugpy port (default: 5678)

# Optional: structured logging
json_log: <boolean>            # Default: false
```

## Defaults

If no `lambdaform.yaml` exists, Lambdaform uses these defaults:

| Setting | Default |
|---------|---------|
| Gateway port | 3000 |
| Hot reload | enabled |
| Debug | disabled |
| JSON logging | disabled |
| CORS | disabled |
| Function timeout | from Terraform (or 3s) |
