# SNS Fanout Example

Demonstrates SNS-triggered Lambda functions with fan-out pattern:
- **orders** topic → 2 consumers (fulfillment + notifier)
- **alerts** topic → 1 consumer (alert handler)
- Multi-runtime: Node.js 20 + Python 3.12

## Test

```bash
# View infrastructure
lambdaform config
lambdaform graph

# Trigger order processing (auto-resolves to fulfillment via subscription)
lambdaform trigger -t sns -s orders -m '{"orderId":"ORD-001","items":["widget"],"customerEmail":"test@example.com"}'

# Trigger specific consumer
lambdaform trigger -t sns -s orders -f order_notifier -m '{"orderId":"ORD-001","customerEmail":"test@example.com"}'

# Trigger alerts
lambdaform trigger -t sns -s alerts -f alert_handler -m '{"alertId":"ALT-42","severity":"critical"}'

# Batch messages
lambdaform trigger -t sns -s orders -m '{"orderId":"ORD-001"}' --batch 5

# Dry run (inspect event payload)
lambdaform trigger -t sns -s orders -m '{"test":true}' --dry-run
```

## Features Tested

- SNS topic parsing (`aws_sns_topic`)
- SNS topic subscription parsing (`aws_sns_topic_subscription`)
- Automatic function resolution via subscriptions
- Fan-out: one topic → multiple Lambda consumers
- Multi-runtime (Node.js + Python) SNS handlers
- Batch message delivery
- `--dry-run` event inspection
- Infrastructure graph with SNS subscription edges
