# SQS Processor Example
# Demonstrates SQS-triggered Lambda with event source mapping,
# dead-letter queue, and batch processing.

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

# ─── SQS Queues ──────────────────────────────────────────────

resource "aws_sqs_queue" "orders_dlq" {
  name                      = "orders-dlq-${var.environment}"
  message_retention_seconds = 1209600 # 14 days
}

resource "aws_sqs_queue" "orders" {
  name                       = "orders-${var.environment}"
  visibility_timeout_seconds = 60
  message_retention_seconds  = 345600 # 4 days
  receive_wait_time_seconds  = 20     # long polling

  redrive_policy = jsonencode({
    deadLetterTargetArn = aws_sqs_queue.orders_dlq.arn
    maxReceiveCount     = 3
  })
}

resource "aws_sqs_queue" "notifications" {
  name = "notifications-${var.environment}"
}

# ─── IAM ─────────────────────────────────────────────────────

resource "aws_iam_role" "lambda_role" {
  name = "sqs-processor-role-${var.environment}"

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

resource "aws_lambda_function" "order_processor" {
  function_name = "order-processor-${var.environment}"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda_role.arn
  timeout       = 30
  memory_size   = 256

  filename = "order_processor.zip"

  environment {
    variables = {
      ENVIRONMENT       = var.environment
      NOTIFICATION_QUEUE = aws_sqs_queue.notifications.url
      DLQ_QUEUE         = aws_sqs_queue.orders_dlq.url
    }
  }
}

resource "aws_lambda_function" "notification_sender" {
  function_name = "notification-sender-${var.environment}"
  handler       = "notify.handler"
  runtime       = "python3.12"
  role          = aws_iam_role.lambda_role.arn
  timeout       = 15

  filename = "notification_sender.zip"

  environment {
    variables = {
      ENVIRONMENT = var.environment
    }
  }
}

# ─── Event Source Mappings ───────────────────────────────────

resource "aws_lambda_event_source_mapping" "orders_to_processor" {
  event_source_arn = aws_sqs_queue.orders.arn
  function_name    = aws_lambda_function.order_processor.arn
  batch_size       = 5
  enabled          = true
}

resource "aws_lambda_event_source_mapping" "notifications_to_sender" {
  event_source_arn = aws_sqs_queue.notifications.arn
  function_name    = aws_lambda_function.notification_sender.arn
  batch_size       = 1
  enabled          = true
}
