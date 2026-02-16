# Request Router

The router (`router.rs`) matches incoming HTTP requests to Lambda functions via API Gateway route definitions.

## Route Matching

### REST API (v1)

Routes are built by traversing `aws_api_gateway_resource` parent chains:

```
root_resource_id
  └── "users" (resource)
       └── "{userId}" (resource)
```

Results in path: `/users/{userId}`

Path parameters (`{param}`) are extracted and passed in `event.pathParameters`.

### HTTP API (v2)

Routes come directly from `aws_apigatewayv2_route.route_key`:

```hcl
route_key = "GET /items/{itemId}"
```

The method and path are parsed from the route key string.

## Matching Priority

When multiple routes could match, Lambdaform uses AWS's priority rules:
1. **Exact match** — `/users/admin` beats `/users/{id}`
2. **Path parameter** — `/users/{id}` beats catch-all
3. **Catch-all** — `$default` route (v2 only)

## Gateway Isolation

Each API Gateway gets its own port. Routes from different gateways never conflict, matching AWS's behavior where each API has an independent URL.
