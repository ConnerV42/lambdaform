# Step Functions

Lambdaform provides read-only visualization of Step Functions state machines defined in your Terraform.

## Usage

```bash
lambdaform stepfunctions
lambdaform sfn              # alias

# Show specific state machine
lambdaform sfn --name my_state_machine

# JSON output
lambdaform sfn --json
```

## Output

Lambdaform parses `aws_sfn_state_machine` resources and renders ASCII flow diagrams:

```
State Machine: order-processor
  [Start] → ValidateOrder
  ValidateOrder → CheckInventory
  CheckInventory → (Choice)
    ├── InStock → ProcessPayment
    └── OutOfStock → NotifyCustomer
  ProcessPayment → ShipOrder
  ShipOrder → [End]
  NotifyCustomer → [End]
```

## Supported States

- **Task** — shows Lambda function reference
- **Choice** — shows branching conditions
- **Wait** — shows delay configuration
- **Parallel** — shows parallel branches
- **Pass** — shows result/input transformations
- **Succeed** / **Fail** — terminal states

## Limitations

This is visualization only — Lambdaform does not execute Step Functions state machines. Use [AWS Step Functions Local](https://docs.aws.amazon.com/step-functions/latest/dg/sfn-local.html) for execution testing.
