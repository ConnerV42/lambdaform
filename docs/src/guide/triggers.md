# SQS & SNS Triggers

Lambdaform simulates SQS and SNS event source mappings, letting you test Lambda triggers locally.

## SQS Triggers

```bash
# Single message
lambdaform trigger sqs my_queue '{"orderId": "123"}'

# Batch of 5 messages
lambdaform trigger sqs my_queue '{"orderId": "123"}' --batch 5

# Target specific function (if multiple subscribers)
lambdaform trigger sqs my_queue '{"orderId": "123"}' --function order_processor
```

The Lambda receives a standard SQS event:

```json
{
  "Records": [{
    "messageId": "local-uuid",
    "receiptHandle": "local-receipt",
    "body": "{\"orderId\": \"123\"}",
    "attributes": {
      "ApproximateReceiveCount": "1",
      "SentTimestamp": "1234567890",
      "SenderId": "local",
      "ApproximateFirstReceiveTimestamp": "1234567890"
    },
    "eventSource": "aws:sqs",
    "eventSourceARN": "arn:aws:sqs:us-east-1:000000000000:my-queue"
  }]
}
```

## SNS Triggers

```bash
lambdaform trigger sns my_topic '{"event": "user_created", "userId": "456"}'
```

SNS event format:

```json
{
  "Records": [{
    "EventSource": "aws:sns",
    "Sns": {
      "Type": "Notification",
      "MessageId": "local-uuid",
      "Message": "{\"event\": \"user_created\", \"userId\": \"456\"}",
      "Timestamp": "2024-01-01T00:00:00.000Z",
      "TopicArn": "arn:aws:sns:us-east-1:000000000000:my-topic"
    }
  }]
}
```

## Terraform Resources

Lambdaform discovers triggers from `aws_lambda_event_source_mapping`:

```hcl
resource "aws_sqs_queue" "orders" {
  name = "order-queue"
}

resource "aws_lambda_event_source_mapping" "sqs_trigger" {
  event_source_arn = aws_sqs_queue.orders.arn
  function_name    = aws_lambda_function.processor.arn
  batch_size       = 10
}
```

The `batch_size` from Terraform is used as the default when `--batch` is not specified.
