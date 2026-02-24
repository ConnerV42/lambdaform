# Step Functions Example

An order processing workflow with 5 Lambda functions and a Step Functions state machine.

## Architecture

The state machine implements an order workflow:
1. **ValidateOrder** (Node.js) — Validates order details and minimum amount
2. **CheckInventory** (Node.js) — Checks if items are in stock
3. **IsInStock** (Choice) — Routes based on inventory result
4. **WaitForRestock** (Wait) — Retries inventory check after 60s
5. **ProcessPayment** (Python) — Charges the customer
6. **ShipAndNotify** (Parallel) — Ships order and notifies customer simultaneously
7. **OrderComplete** (Succeed) / **OrderFailed** (Fail)

## Usage

```bash
# Visualize the state machine
lambdaform sfn

# View as JSON
lambdaform sfn --json

# Invoke individual functions
lambdaform invoke validate-order -e '{"orderId":"ORD-001","amount":50,"items":[{"name":"Widget","quantity":2}]}'
lambdaform invoke check-inventory -e '{"orderId":"ORD-001","items":[{"name":"Widget","quantity":2}]}'
lambdaform invoke process-payment -e '{"orderId":"ORD-001","amount":50}'
```

## Features Tested

- Step Functions ASL parsing from `jsonencode()` in Terraform
- ASCII flow diagram rendering (Choice branches, Parallel, Wait, loops)
- Direct Lambda invocation with raw events
- Mixed runtimes (Node.js 20 + Python 3.12)
- Environment variables
- Retry and Catch error handling in ASL
