# Debugging

Lambdaform integrates with Node.js Inspector and Python debugpy for step-through debugging.

## Node.js

```bash
lambdaform start --debug
# or with custom port:
lambdaform start --debug --debug-port 9230
```

Attach with VS Code (`launch.json`):

```json
{
  "type": "node",
  "request": "attach",
  "name": "Attach to Lambdaform",
  "port": 9229,
  "restart": true,
  "skipFiles": ["<node_internals>/**"]
}
```

Or open `chrome://inspect` in Chrome and connect to `localhost:9229`.

## Python

```bash
lambdaform start --debug-python
# or with custom port:
lambdaform start --debug-python --debug-python-port 5679
```

Requires `debugpy` installed in your Python environment:
```bash
pip install debugpy
```

Attach with VS Code:

```json
{
  "type": "debugpy",
  "request": "attach",
  "name": "Attach to Lambdaform",
  "connect": { "host": "localhost", "port": 5678 }
}
```

## Important Notes

- **Debug mode disables the warm process pool.** Each invocation spawns a fresh process so breakpoints trigger reliably.
- **Performance is slower** in debug mode due to single-process execution.
- **Both debuggers can run simultaneously** (`--debug --debug-python`) on different ports.
