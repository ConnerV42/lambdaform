# Multiple Gateways

Projects with multiple API Gateways get separate ports automatically.

## Automatic Port Assignment

```
🌐 public-api → http://localhost:3000
🌐 admin-api  → http://localhost:3001
🌐 websocket  → ws://localhost:3002
```

Ports are assigned sequentially starting from the configured base port (default: 3000).

## Manual Port Assignment

Override in `lambdaform.yaml`:

```yaml
gateways:
  public_api:      # Terraform resource name
    port: 8080
  admin_api:
    port: 8081
  websocket_api:
    port: 9000
```

## Identifying Gateways

Use `lambdaform config` to see which port maps to which gateway:

```bash
lambdaform config
```

Each gateway's routes are listed with their assigned port.
