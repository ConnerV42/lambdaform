# Request History & Replay

Record HTTP requests and replay them for testing and debugging.

## Recording Requests

Start the server with recording enabled:

```bash
lambdaform start --record
```

All incoming requests are saved to `.lambdaform-history.jsonl` in the project directory.

## Listing Recorded Requests

```bash
lambdaform replay --list
```

```
Recorded requests:
  [0] GET /users (200) — 2024-01-15 10:30:45
  [1] POST /users (201) — 2024-01-15 10:31:02
  [2] GET /users/123 (200) — 2024-01-15 10:31:15
```

## Replaying Requests

```bash
# Replay a specific request
lambdaform replay --id 2

# Replay the last 5 requests
lambdaform replay --last 5

# Replay all recorded requests
lambdaform replay --all
```

> **Note:** The server must be running for replay to work. Replay sends real HTTP requests to the local server.

## Filtering

```bash
# Only requests matching a path prefix
lambdaform replay --list --filter /users

# Only GET requests
lambdaform replay --list --filter-method GET

# Combined
lambdaform replay --all --filter /api --filter-method POST
```

## Use Cases

- **Regression testing:** Record a session, make code changes, replay to verify behavior
- **Debugging:** Replay a failing request with `--debug` mode enabled
- **Demo:** Record a scripted sequence and replay it during presentations
