terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

provider "aws" {
  region = "us-west-2"
}

# IAM Role for Lambda
resource "aws_iam_role" "lambda_role" {
  name = "order-workflow-lambda-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "lambda.amazonaws.com" }
    }]
  })
}

# IAM Role for Step Functions
resource "aws_iam_role" "sfn_role" {
  name = "order-workflow-sfn-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "states.amazonaws.com" }
    }]
  })
}

# --- Lambda Functions ---

resource "aws_lambda_function" "validate_order" {
  function_name = "validate-order"
  handler       = "validate.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda_role.arn
  filename      = "src/validate.js"

  environment {
    variables = {
      MIN_ORDER_AMOUNT = "10"
    }
  }
}

resource "aws_lambda_function" "check_inventory" {
  function_name = "check-inventory"
  handler       = "inventory.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda_role.arn
  filename      = "src/inventory.js"
}

resource "aws_lambda_function" "process_payment" {
  function_name = "process-payment"
  handler       = "payment.handler"
  runtime       = "python3.12"
  role          = aws_iam_role.lambda_role.arn
  filename      = "src/payment.py"

  timeout = 30

  environment {
    variables = {
      PAYMENT_GATEWAY_URL = "https://api.stripe.example.com"
    }
  }
}

resource "aws_lambda_function" "ship_order" {
  function_name = "ship-order"
  handler       = "shipping.handler"
  runtime       = "python3.12"
  role          = aws_iam_role.lambda_role.arn
  filename      = "src/shipping.py"
}

resource "aws_lambda_function" "notify_customer" {
  function_name = "notify-customer"
  handler       = "notify.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda_role.arn
  filename      = "src/notify.js"
}

# --- Step Functions State Machine ---

resource "aws_sfn_state_machine" "order_workflow" {
  name     = "order-processing-workflow"
  role_arn = aws_iam_role.sfn_role.arn
  type     = "STANDARD"

  definition = jsonencode({
    Comment  = "Order processing workflow with validation, payment, and shipping"
    StartAt  = "ValidateOrder"
    TimeoutSeconds = 300
    States = {
      ValidateOrder = {
        Type     = "Task"
        Resource = aws_lambda_function.validate_order.arn
        Comment  = "Validate order details and amount"
        Next     = "CheckInventory"
        Retry = [{
          ErrorEquals = ["States.TaskFailed"]
          MaxAttempts = 2
        }]
        Catch = [{
          ErrorEquals = ["ValidationError"]
          Next        = "OrderFailed"
        }]
      }

      CheckInventory = {
        Type     = "Task"
        Resource = aws_lambda_function.check_inventory.arn
        Comment  = "Check if items are in stock"
        Next     = "IsInStock"
      }

      IsInStock = {
        Type = "Choice"
        Choices = [{
          Variable     = "$.inStock"
          BooleanEquals = true
          Next         = "ProcessPayment"
        }]
        Default = "WaitForRestock"
      }

      WaitForRestock = {
        Type    = "Wait"
        Seconds = 60
        Next    = "CheckInventory"
      }

      ProcessPayment = {
        Type     = "Task"
        Resource = aws_lambda_function.process_payment.arn
        Comment  = "Charge customer payment method"
        Next     = "ShipAndNotify"
        Retry = [{
          ErrorEquals = ["PaymentRetryable"]
          MaxAttempts = 3
        }]
        Catch = [{
          ErrorEquals = ["PaymentFailed"]
          Next        = "OrderFailed"
        }]
      }

      ShipAndNotify = {
        Type = "Parallel"
        Branches = [
          {
            StartAt = "ShipOrder"
            States = {
              ShipOrder = {
                Type     = "Task"
                Resource = aws_lambda_function.ship_order.arn
                End      = true
              }
            }
          },
          {
            StartAt = "NotifyCustomer"
            States = {
              NotifyCustomer = {
                Type     = "Task"
                Resource = aws_lambda_function.notify_customer.arn
                End      = true
              }
            }
          }
        ]
        Next = "OrderComplete"
      }

      OrderComplete = {
        Type = "Succeed"
      }

      OrderFailed = {
        Type  = "Fail"
        Cause = "Order processing failed"
        Error = "OrderError"
      }
    }
  })
}
