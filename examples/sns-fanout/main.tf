# SNS Fanout Example
# Demonstrates SNS-triggered Lambda with topic subscriptions,
# fan-out to multiple consumers, and message filtering.

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

variable "environment" {
  default = "dev"
}

# ─── SNS Topics ──────────────────────────────────────────────

resource "aws_sns_topic" "orders" {
  name = "orders-${var.environment}"
}

resource "aws_sns_topic" "alerts" {
  name = "system-alerts-${var.environment}"
}

# ─── IAM ─────────────────────────────────────────────────────

resource "aws_iam_role" "lambda_role" {
  name = "sns-fanout-role-${var.environment}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "lambda.amazonaws.com" }
    }]
  })
}

# ─── Lambda Functions ────────────────────────────────────────

# Consumer 1: Processes orders (e.g., fulfillment)
resource "aws_lambda_function" "order_fulfillment" {
  function_name = "order-fulfillment-${var.environment}"
  handler       = "fulfillment.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda_role.arn
  timeout       = 30
  memory_size   = 256

  filename = "fulfillment.zip"

  environment {
    variables = {
      ENVIRONMENT = var.environment
      SERVICE     = "fulfillment"
    }
  }
}

# Consumer 2: Sends order notifications (e.g., email/SMS)
resource "aws_lambda_function" "order_notifier" {
  function_name = "order-notifier-${var.environment}"
  handler       = "notifier.handler"
  runtime       = "python3.12"
  role          = aws_iam_role.lambda_role.arn
  timeout       = 15

  filename = "notifier.zip"

  environment {
    variables = {
      ENVIRONMENT = var.environment
      SERVICE     = "notifier"
    }
  }
}

# Consumer 3: Handles system alerts
resource "aws_lambda_function" "alert_handler" {
  function_name = "alert-handler-${var.environment}"
  handler       = "alerts.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda_role.arn
  timeout       = 10

  filename = "alerts.zip"

  environment {
    variables = {
      ENVIRONMENT = var.environment
      SERVICE     = "alerts"
    }
  }
}

# ─── SNS Subscriptions ──────────────────────────────────────

resource "aws_sns_topic_subscription" "orders_to_fulfillment" {
  topic_arn = aws_sns_topic.orders.arn
  protocol  = "lambda"
  endpoint  = aws_lambda_function.order_fulfillment.arn
}

resource "aws_sns_topic_subscription" "orders_to_notifier" {
  topic_arn = aws_sns_topic.orders.arn
  protocol  = "lambda"
  endpoint  = aws_lambda_function.order_notifier.arn
}

resource "aws_sns_topic_subscription" "alerts_to_handler" {
  topic_arn = aws_sns_topic.alerts.arn
  protocol  = "lambda"
  endpoint  = aws_lambda_function.alert_handler.arn
}
