# Event Formats

Lambdaform generates the correct event format for each trigger type.

## API Gateway REST (v1)

```json
{
  "resource": "/users/{userId}",
  "path": "/users/123",
  "httpMethod": "GET",
  "headers": {
    "Content-Type": "application/json",
    "Host": "localhost:3000"
  },
  "queryStringParameters": { "include": "profile" },
  "pathParameters": { "userId": "123" },
  "body": null,
  "isBase64Encoded": false,
  "requestContext": {
    "resourceId": "local",
    "resourcePath": "/users/{userId}",
    "httpMethod": "GET",
    "requestId": "local-uuid",
    "apiId": "local",
    "stage": "local"
  }
}
```

## API Gateway HTTP (v2)

```json
{
  "version": "2.0",
  "rawPath": "/users/123",
  "rawQueryString": "include=profile",
  "headers": {
    "content-type": "application/json"
  },
  "queryStringParameters": { "include": "profile" },
  "pathParameters": { "userId": "123" },
  "body": null,
  "isBase64Encoded": false,
  "requestContext": {
    "http": {
      "method": "GET",
      "path": "/users/123"
    },
    "requestId": "local-uuid",
    "apiId": "local",
    "stage": "$default"
  }
}
```

## WebSocket

```json
{
  "requestContext": {
    "routeKey": "$connect",
    "connectionId": "local-conn-id",
    "eventType": "CONNECT",
    "apiId": "local",
    "stage": "local"
  },
  "headers": { ... },
  "isBase64Encoded": false
}
```

For message routes, `body` contains the message payload.

## SQS

```json
{
  "Records": [{
    "messageId": "local-uuid",
    "receiptHandle": "local-receipt",
    "body": "{\"key\": \"value\"}",
    "attributes": {
      "ApproximateReceiveCount": "1",
      "SentTimestamp": "1234567890",
      "SenderId": "local",
      "ApproximateFirstReceiveTimestamp": "1234567890"
    },
    "eventSource": "aws:sqs",
    "eventSourceARN": "arn:aws:sqs:us-east-1:000000000000:queue-name",
    "awsRegion": "us-east-1"
  }]
}
```

## SNS

```json
{
  "Records": [{
    "EventSource": "aws:sns",
    "EventSubscriptionArn": "arn:aws:sns:us-east-1:000000000000:topic:local-sub",
    "Sns": {
      "Type": "Notification",
      "MessageId": "local-uuid",
      "TopicArn": "arn:aws:sns:us-east-1:000000000000:topic-name",
      "Message": "{\"key\": \"value\"}",
      "Timestamp": "2024-01-01T00:00:00.000Z"
    }
  }]
}
```

## Direct Invoke

When using `lambdaform invoke`, the event is passed directly as provided:

```bash
lambdaform invoke my_function --event '{"custom": "payload"}'
```

The JSON is sent to the handler as-is, with no wrapping.
