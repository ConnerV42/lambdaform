# Lambda Authorizers

Lambdaform executes Lambda authorizers before your handler, just like AWS does. Both v1 and v2 authorizer types are supported.

## REST API (v1) Authorizers

### TOKEN Authorizer

Extracts a token from a specified header and passes it to the authorizer function:

```hcl
resource "aws_api_gateway_authorizer" "token_auth" {
  name            = "token-auth"
  rest_api_id     = aws_api_gateway_rest_api.api.id
  type            = "TOKEN"
  authorizer_uri  = aws_lambda_function.authorizer.invoke_arn
  identity_source = "method.request.header.Authorization"
}

resource "aws_api_gateway_method" "protected" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.data.id
  http_method   = "GET"
  authorization = "CUSTOM"
  authorizer_id = aws_api_gateway_authorizer.token_auth.id
}
```

The authorizer Lambda receives:

```json
{
  "type": "TOKEN",
  "authorizationToken": "Bearer eyJ...",
  "methodArn": "arn:aws:execute-api:us-east-1:123456789:api-id/local/GET/data"
}
```

### REQUEST Authorizer

Passes the full request context (headers, query params, path params) to the authorizer:

```hcl
resource "aws_api_gateway_authorizer" "request_auth" {
  name            = "request-auth"
  rest_api_id     = aws_api_gateway_rest_api.api.id
  type            = "REQUEST"
  authorizer_uri  = aws_lambda_function.authorizer.invoke_arn
}
```

## HTTP API (v2) Authorizers

```hcl
resource "aws_apigatewayv2_authorizer" "auth" {
  api_id           = aws_apigatewayv2_api.api.id
  authorizer_type  = "REQUEST"
  authorizer_uri   = aws_lambda_function.authorizer.invoke_arn
  name             = "auth"
}
```

## Authorizer Response

Your authorizer must return an IAM policy document:

```javascript
exports.handler = async (event) => {
  const token = event.authorizationToken || event.headers?.authorization;

  if (isValid(token)) {
    return {
      principalId: "user123",
      policyDocument: {
        Version: "2012-10-17",
        Statement: [{
          Action: "execute-api:Invoke",
          Effect: "Allow",
          Resource: event.methodArn || "*"
        }]
      }
    };
  }

  throw new Error("Unauthorized");
};
```

## Behavior

- **Allow:** Authorizer returns `Effect: "Allow"` → request proceeds to handler
- **Deny:** Authorizer returns `Effect: "Deny"` → `403 Forbidden`
- **Error:** Authorizer throws → `401 Unauthorized`
- **Missing token:** No token in identity source → `401 Unauthorized`

> **Note:** Authorizer result caching is not implemented locally. Every request runs the authorizer.
