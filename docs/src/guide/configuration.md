# Configuration

Lambdaform works without configuration — it reads your Terraform directly. For cases where you need to override behavior, create a `lambdaform.yaml` in your project root.

## Full Example

```yaml
version: 1

# Override function settings
functions:
  api_handler:
    source: ./dist               # Override source directory
    handler: build/index.handler  # Override handler path
    timeout: 30                   # Seconds
    memory: 256                   # MB (informational)
    environment:
      TABLE_NAME: local-table    # Override/add env vars

# Global environment variables (applied to all functions)
environment:
  AWS_REGION: us-west-2
  STAGE: local

# Default gateway settings
gateway:
  port: 3000

# Per-gateway port overrides
gateways:
  public_api:
    port: 3000
  admin_api:
    port: 3001

# CORS configuration
cors:
  allow_origins:
    - "http://localhost:5173"
    - "http://localhost:3001"
  allow_methods:
    - GET
    - POST
    - PUT
    - DELETE
    - OPTIONS
  allow_headers:
    - Content-Type
    - Authorization

# File watcher settings
watch:
  enabled: true
  ignore:
    - node_modules
    - .terraform
    - dist

# Debugger settings
debug:
  enabled: false
  port: 9229            # Node.js inspector port
  python: false
  python_port: 5678     # debugpy port
```

## Sections

### `functions`

Override per-function settings. Keys are Terraform resource names (not `function_name`):

```yaml
functions:
  my_lambda_resource:     # matches: resource "aws_lambda_function" "my_lambda_resource"
    source: ./build       # where to find handler code
    handler: app.handler  # override handler
    timeout: 60
    environment:
      DEBUG: "true"
```

Environment variables from the config file are merged with those from Terraform. Config values take precedence.

### `environment`

Global environment variables applied to all functions:

```yaml
environment:
  AWS_REGION: us-west-2
  DYNAMODB_ENDPOINT: http://localhost:8000
```

Precedence (highest wins):
1. Function-specific `environment` in config
2. Global `environment` in config
3. `environment.variables` from Terraform

### `gateway` / `gateways`

Default port and per-gateway overrides:

```yaml
gateway:
  port: 8080          # default port for all gateways

gateways:
  api_v1:             # Terraform resource name
    port: 8080
  api_v2:
    port: 8081
```

### `cors`

CORS headers applied to all responses. Also handles `OPTIONS` preflight requests automatically:

```yaml
cors:
  allow_origins: ["*"]
  allow_methods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"]
  allow_headers: ["Content-Type", "Authorization"]
```

### `watch`

Control the file watcher for hot reload:

```yaml
watch:
  enabled: true         # default: true
  ignore:
    - node_modules
    - .git
    - __pycache__
```

### `debug`

Debugger configuration (can also be set via CLI flags):

```yaml
debug:
  enabled: true
  port: 9229
  python: true
  python_port: 5678
```

> **Note:** Debug mode disables the warm process pool so breakpoints work correctly.

## Config File Location

Lambdaform looks for `lambdaform.yaml` (or `lambdaform.yml`) in the target directory (`.` or the `--dir` path). Generate one with:

```bash
lambdaform init
```
