# Lambda Function URLs

Lambda Function URLs provide a dedicated HTTP endpoint for a Lambda function without requiring API Gateway. Lambdaform automatically detects `aws_lambda_function_url` resources in your Terraform configuration and serves each one on its own port.

## How It Works

When Lambdaform finds an `aws_lambda_function_url` resource, it:

1. Binds the function to a dedicated port (separate from API Gateway ports)
2. Applies CORS configuration from the Terraform resource
3. Routes all HTTP methods and paths to that single function
4. Sends events in the Function URL event format (similar to API Gateway v2)

## Terraform Configuration

```hcl
resource "aws_lambda_function" "my_api" {
  function_name = "my-api"
  runtime       = "nodejs20.x"
  handler       = "index.handler"
  filename      = "lambda.zip"
}

resource "aws_lambda_function_url" "my_api_url" {
  function_name      = aws_lambda_function.my_api.function_name
  authorization_type = "NONE"

  cors {
    allow_origins  = ["https://example.com"]
    allow_methods  = ["GET", "POST", "PUT", "DELETE"]
    allow_headers  = ["Content-Type", "Authorization"]
    expose_headers = ["X-Request-Id"]
    max_age        = 3600
    allow_credentials = true
  }
}
```

## Starting the Server

```bash
lambdaform start
```

Lambdaform displays the Function URL endpoint in the startup output:

```
🚀 Lambdaform v1.0.0
  Functions: 1 discovered
  API Gateway: (none)
  Function URLs:
    my_api_url → http://localhost:3001 (my-api)
  Watching for changes...
```

## Event Format

Function URL events follow the Lambda Function URL payload format, which is similar to API Gateway v2 (HTTP API) events:

```json
{
  "version": "2.0",
  "routeKey": "$default",
  "rawPath": "/hello",
  "rawQueryString": "name=world",
  "headers": {
    "content-type": "application/json"
  },
  "queryStringParameters": {
    "name": "world"
  },
  "requestContext": {
    "http": {
      "method": "GET",
      "path": "/hello",
      "protocol": "HTTP/1.1",
      "sourceIp": "127.0.0.1",
      "userAgent": "curl/8.0"
    },
    "requestId": "...",
    "time": "2026-01-15T10:30:00Z",
    "timeEpoch": 1768476600000
  },
  "isBase64Encoded": false
}
```

## Handler Example

Your Lambda handler works the same as it would with a real Function URL:

```javascript
// Node.js
export const handler = async (event) => {
  return {
    statusCode: 200,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      message: "Hello from Function URL!",
      path: event.rawPath,
      method: event.requestContext.http.method,
    }),
  };
};
```

```python
# Python
def handler(event, context):
    return {
        "statusCode": 200,
        "headers": {"Content-Type": "application/json"},
        "body": json.dumps({
            "message": "Hello from Function URL!",
            "path": event["rawPath"],
            "method": event["requestContext"]["http"]["method"],
        }),
    }
```

## CORS

CORS is configured directly in the Terraform `aws_lambda_function_url` resource's `cors` block. Lambdaform reads this configuration and applies it automatically — no need for manual CORS headers in your handler.

If `authorization_type` is `"NONE"`, preflight `OPTIONS` requests are handled automatically by the CORS layer.

## Authorization Types

| Type | Behavior in Lambdaform |
|------|----------------------|
| `NONE` | All requests pass through (no auth check) |
| `AWS_IAM` | Treated as `NONE` locally (IAM auth not simulated) |

## Function URLs vs API Gateway

| Feature | Function URL | API Gateway |
|---------|-------------|-------------|
| Routing | Single function, all paths | Multiple functions, path-based routing |
| CORS | Built-in Terraform config | CorsLayer or manual headers |
| Auth | IAM or NONE | Lambda authorizers, IAM, Cognito |
| Cost | Free (included with Lambda) | Per-request pricing |
| Use case | Simple single-function APIs, webhooks | Multi-function APIs, complex routing |

Both work seamlessly with Lambdaform's hot reload and request recording.
