# SQS Processor Example

Demonstrates SQS-triggered Lambda functions with Lambdaform's `trigger` command.

## Architecture

```
orders queue → order_processor (Node.js) → processes orders, reports partial failures
notifications queue → notification_sender (Python) → sends notifications
orders_dlq → dead letter queue for failed orders (after 3 retries)
```

## Usage

```bash
# Start Lambdaform
lambdaform start

# Send a single order
lambdaform trigger -t sqs -s orders \
  -m '{"orderId": "ORD-001", "items": [{"name": "Widget", "price": 9.99, "quantity": 2}]}'

# Send a batch of 3 identical orders
lambdaform trigger -t sqs -s orders \
  -m '{"orderId": "ORD-002", "items": [{"name": "Gadget", "price": 24.99}]}' \
  --batch 3

# Send a notification (routes to Python handler)
lambdaform trigger -t sqs -s notifications \
  -m '{"type": "email", "recipient": "user@example.com", "message": "Your order shipped!"}'

# Send an invalid order (demonstrates partial batch failure)
lambdaform trigger -t sqs -s orders -m '{"bad": "data"}'
```

## Features Demonstrated

- **SQS event source mappings** — Terraform `aws_lambda_event_source_mapping`
- **Batch processing** — Multiple messages in one invocation
- **Partial batch failures** — `batchItemFailures` response
- **Dead letter queue** — `redrive_policy` with DLQ
- **Multi-runtime** — Node.js processor + Python sender
- **Environment variables** — Queue URLs passed via env vars
