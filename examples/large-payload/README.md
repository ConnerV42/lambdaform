# Large Payload Example

Tests request body size limits, binary payloads, and base64 encoding — the edge cases that trip up most Lambda emulators.

## What This Tests

| Feature | Endpoint | Expected |
|---------|----------|----------|
| Large JSON body (~5MB) | POST /echo | Receives full body, returns size |
| Binary upload (octet-stream) | POST /binary | Base64-encoded in event, binary response |
| Image upload | POST /image | Detects image type from magic bytes |
| Over-limit payload (>10MB) | POST /echo | 413 Payload Too Large |
| Function URL 6MB limit | Function URL /binary | 413 if >6MB |

## Run

```bash
cd examples/large-payload
lambdaform start

# 1. Normal JSON payload
curl -X POST http://localhost:3000/echo \
  -H 'Content-Type: application/json' \
  -d '{"message": "hello"}'

# 2. Large payload (~1MB)
dd if=/dev/urandom bs=1024 count=1024 2>/dev/null | base64 | \
  curl -X POST http://localhost:3000/echo \
  -H 'Content-Type: text/plain' \
  --data-binary @-

# 3. Binary payload
dd if=/dev/urandom bs=1024 count=100 2>/dev/null | \
  curl -X POST http://localhost:3000/binary \
  -H 'Content-Type: application/octet-stream' \
  --data-binary @-

# 4. Fake PNG image
printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR' | \
  curl -X POST http://localhost:3000/image \
  -H 'Content-Type: image/png' \
  --data-binary @-

# 5. Over-limit (>10MB) — should get 413
dd if=/dev/urandom bs=1024 count=11264 2>/dev/null | \
  curl -X POST http://localhost:3000/echo \
  -H 'Content-Type: application/octet-stream' \
  --data-binary @-
```
