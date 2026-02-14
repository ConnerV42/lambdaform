# SQS/SNS trigger test fixture

resource "aws_sqs_queue" "orders" {
  name = "orders-queue"
  visibility_timeout_seconds = 60
}

resource "aws_sqs_queue" "notifications" {
  name                        = "notifications.fifo"
  fifo_queue                  = true
  visibility_timeout_seconds  = 30
}

resource "aws_sns_topic" "alerts" {
  name = "alerts-topic"
}

resource "aws_sns_topic" "events" {
  name       = "events.fifo"
  fifo_topic = true
}

resource "aws_lambda_function" "order_processor" {
  function_name = "order-processor"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "."

  environment {
    variables = {
      QUEUE_URL = "http://localhost:4566/000000000000/orders-queue"
    }
  }
}

resource "aws_lambda_function" "alert_handler" {
  function_name = "alert-handler"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "."
}

resource "aws_lambda_event_source_mapping" "orders_to_processor" {
  event_source_arn = aws_sqs_queue.orders.arn
  function_name    = aws_lambda_function.order_processor.arn
  batch_size       = 5
  enabled          = true
}

resource "aws_lambda_event_source_mapping" "fifo_to_processor" {
  event_source_arn = aws_sqs_queue.notifications.arn
  function_name    = aws_lambda_function.order_processor.arn
  batch_size       = 1
  enabled          = true
}
