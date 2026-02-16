# Hot Reload

Lambdaform watches your project files and automatically reloads when changes are detected. No server restart required.

## What's Watched

- **Lambda handler code** (`.js`, `.py`, `.go` files)
- **Terraform files** (`.tf`, `.tfvars`)
- **Config file** (`lambdaform.yaml`)

## What Happens on Reload

1. **Code changes:** Warm process pool is flushed. Next invocation uses fresh code.
2. **Terraform changes:** Full re-parse of `.tf` files. Routes and functions are updated.
3. **Config changes:** Configuration is re-read and applied.

## Ignored Paths

By default, these directories are ignored:
- `node_modules/`
- `.terraform/`
- `.git/`
- `__pycache__/`

Add custom ignores in `lambdaform.yaml`:

```yaml
watch:
  ignore:
    - dist
    - build
    - .next
```

## Disabling Hot Reload

```bash
lambdaform start --no-watch
```

Or in config:

```yaml
watch:
  enabled: false
```
