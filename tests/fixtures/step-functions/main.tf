provider "aws" {
  region = "us-west-2"
}

# Lambda functions used by the state machine
resource "aws_lambda_function" "validate_order" {
  function_name = "validate-order"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "validate.zip"
  role          = aws_iam_role.lambda.arn
}

resource "aws_lambda_function" "process_payment" {
  function_name = "process-payment"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "payment.zip"
  role          = aws_iam_role.lambda.arn
}

resource "aws_lambda_function" "ship_order" {
  function_name = "ship-order"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "ship.zip"
  role          = aws_iam_role.lambda.arn
}

resource "aws_lambda_function" "notify_failure" {
  function_name = "notify-failure"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "notify.zip"
  role          = aws_iam_role.lambda.arn
}

# Step Functions state machine
resource "aws_sfn_state_machine" "order_workflow" {
  name     = "order-processing-workflow"
  role_arn = aws_iam_role.sfn.arn
  type     = "STANDARD"

  definition = <<EOF
{
  "Comment": "Order processing workflow with payment and shipping",
  "StartAt": "ValidateOrder",
  "TimeoutSeconds": 300,
  "States": {
    "ValidateOrder": {
      "Type": "Task",
      "Resource": "arn:aws:lambda:us-west-2:123456789:function:validate-order",
      "Comment": "Validate the incoming order",
      "Next": "CheckInventory",
      "Retry": [
        {
          "ErrorEquals": ["States.TaskFailed"],
          "MaxAttempts": 2
        }
      ]
    },
    "CheckInventory": {
      "Type": "Task",
      "Resource": "arn:aws:lambda:us-west-2:123456789:function:check-inventory",
      "Next": "IsInStock"
    },
    "IsInStock": {
      "Type": "Choice",
      "Choices": [
        {
          "Variable": "$.inStock",
          "BooleanEquals": true,
          "Next": "ProcessPayment"
        }
      ],
      "Default": "NotifyOutOfStock"
    },
    "ProcessPayment": {
      "Type": "Task",
      "Resource": "arn:aws:lambda:us-west-2:123456789:function:process-payment",
      "Retry": [
        {
          "ErrorEquals": ["PaymentTimeout"],
          "MaxAttempts": 3
        }
      ],
      "Catch": [
        {
          "ErrorEquals": ["PaymentFailed"],
          "Next": "NotifyPaymentFailure"
        }
      ],
      "Next": "WaitForConfirmation"
    },
    "WaitForConfirmation": {
      "Type": "Wait",
      "Seconds": 5,
      "Next": "ShipOrder"
    },
    "ShipOrder": {
      "Type": "Task",
      "Resource": "arn:aws:lambda:us-west-2:123456789:function:ship-order",
      "End": true
    },
    "NotifyOutOfStock": {
      "Type": "Task",
      "Resource": "arn:aws:lambda:us-west-2:123456789:function:notify-failure",
      "End": true
    },
    "NotifyPaymentFailure": {
      "Type": "Fail",
      "Cause": "Payment could not be processed",
      "Error": "PaymentError"
    }
  }
}
EOF
}

# Express state machine for quick transforms
resource "aws_sfn_state_machine" "data_transform" {
  name     = "data-transform"
  role_arn = aws_iam_role.sfn.arn
  type     = "EXPRESS"

  definition = <<EOF
{
  "Comment": "Quick data transformation pipeline",
  "StartAt": "Transform",
  "States": {
    "Transform": {
      "Type": "Pass",
      "Result": {"status": "transformed"},
      "Next": "Done"
    },
    "Done": {
      "Type": "Succeed"
    }
  }
}
EOF
}
