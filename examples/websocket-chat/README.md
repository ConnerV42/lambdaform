# WebSocket Chat

A real-time chat example using WebSocket API Gateway with Lambdaform.

## Routes

| Route Key      | Handler          | Description                     |
|----------------|------------------|---------------------------------|
| `$connect`     | connect.handler  | Client connection               |
| `$disconnect`  | disconnect.handler | Client disconnection          |
| `$default`     | default.handler  | Catch-all (echo)                |
| `sendmessage`  | sendmessage.handler | Broadcast via @connections API |

## Run locally

```bash
cd examples/websocket-chat
lambdaform start
```

## Test with wscat

```bash
# Terminal 1: connect
wscat -c ws://localhost:3000

# Send a message (routed to sendmessage handler)
> {"action": "sendmessage", "data": "Hello!"}

# Send unknown action (routed to $default)
> {"action": "unknown"}

# Plain text (routed to $default)
> hello
```
