# WebSocket APIs

Lambdaform supports WebSocket API Gateway (`aws_apigatewayv2_api` with `protocol_type = "WEBSOCKET"`).

## Terraform Setup

```hcl
resource "aws_apigatewayv2_api" "ws" {
  name                       = "websocket-api"
  protocol_type              = "WEBSOCKET"
  route_selection_expression = "$request.body.action"
}

resource "aws_apigatewayv2_route" "connect" {
  api_id    = aws_apigatewayv2_api.ws.id
  route_key = "$connect"
  target    = "integrations/${aws_apigatewayv2_integration.connect.id}"
}

resource "aws_apigatewayv2_route" "disconnect" {
  api_id    = aws_apigatewayv2_api.ws.id
  route_key = "$disconnect"
  target    = "integrations/${aws_apigatewayv2_integration.disconnect.id}"
}

resource "aws_apigatewayv2_route" "default" {
  api_id    = aws_apigatewayv2_api.ws.id
  route_key = "$default"
  target    = "integrations/${aws_apigatewayv2_integration.default.id}"
}

resource "aws_apigatewayv2_route" "chat" {
  api_id    = aws_apigatewayv2_api.ws.id
  route_key = "chat"
  target    = "integrations/${aws_apigatewayv2_integration.chat.id}"
}
```

## Built-in Routes

| Route | Triggered |
|-------|-----------|
| `$connect` | When a WebSocket client connects |
| `$disconnect` | When a client disconnects |
| `$default` | For messages that don't match a custom route |

## Custom Routes

The `route_selection_expression` determines which route handles a message. With `$request.body.action`, a message like `{"action": "chat", "message": "hello"}` routes to the `chat` route.

## @connections API

Lambdaform provides a local `@connections` management API. Your Lambda can post messages back to connected clients:

```javascript
const { ApiGatewayManagementApiClient, PostToConnectionCommand } = require("@aws-sdk/client-apigatewaymanagementapi");

const client = new ApiGatewayManagementApiClient({
  endpoint: `http://localhost:3001`,  // Lambdaform WS management endpoint
});

await client.send(new PostToConnectionCommand({
  ConnectionId: event.requestContext.connectionId,
  Data: JSON.stringify({ message: "Hello!" }),
}));
```

## Event Format

WebSocket events use the proper `WebSocketEvent` format:

```json
{
  "requestContext": {
    "routeKey": "$connect",
    "connectionId": "abc123",
    "eventType": "CONNECT",
    "apiId": "local"
  },
  "headers": { ... },
  "isBase64Encoded": false
}
```

## Testing

Use `wscat` or any WebSocket client:

```bash
npm install -g wscat
wscat -c ws://localhost:3001
> {"action": "chat", "message": "hello"}
```
