# WebSocket Chat — Lambdaform dogfooding example
# Tests: $connect/$disconnect/$default routes, custom routes, @connections API

resource "aws_apigatewayv2_api" "chat" {
  name                       = "websocket-chat"
  protocol_type              = "WEBSOCKET"
  route_selection_expression = "$request.body.action"
}

resource "aws_apigatewayv2_stage" "live" {
  api_id = aws_apigatewayv2_api.chat.id
  name   = "live"
}

# --- Lambda functions ---

resource "aws_lambda_function" "connect" {
  function_name = "chat-connect"
  handler       = "connect.handler"
  runtime       = "nodejs20.x"
  filename      = "src/connect.js"
  role          = aws_iam_role.lambda.arn
  timeout       = 10
  memory_size   = 128
}

resource "aws_lambda_function" "disconnect" {
  function_name = "chat-disconnect"
  handler       = "disconnect.handler"
  runtime       = "nodejs20.x"
  filename      = "src/disconnect.js"
  role          = aws_iam_role.lambda.arn
  timeout       = 10
  memory_size   = 128
}

resource "aws_lambda_function" "default_handler" {
  function_name = "chat-default"
  handler       = "default.handler"
  runtime       = "nodejs20.x"
  filename      = "src/default.js"
  role          = aws_iam_role.lambda.arn
  timeout       = 10
  memory_size   = 128
}

resource "aws_lambda_function" "sendmessage" {
  function_name = "chat-sendmessage"
  handler       = "sendmessage.handler"
  runtime       = "nodejs20.x"
  filename      = "src/sendmessage.js"
  role          = aws_iam_role.lambda.arn
  timeout       = 10
  memory_size   = 128

  environment {
    variables = {
      CONNECTIONS_URL = "http://localhost:3201"
    }
  }
}

# --- Routes ---

resource "aws_apigatewayv2_route" "connect" {
  api_id    = aws_apigatewayv2_api.chat.id
  route_key = "$connect"
  target    = "integrations/${aws_apigatewayv2_integration.connect.id}"
}

resource "aws_apigatewayv2_integration" "connect" {
  api_id             = aws_apigatewayv2_api.chat.id
  integration_type   = "AWS_PROXY"
  integration_uri    = aws_lambda_function.connect.invoke_arn
}

resource "aws_apigatewayv2_route" "disconnect" {
  api_id    = aws_apigatewayv2_api.chat.id
  route_key = "$disconnect"
  target    = "integrations/${aws_apigatewayv2_integration.disconnect.id}"
}

resource "aws_apigatewayv2_integration" "disconnect" {
  api_id             = aws_apigatewayv2_api.chat.id
  integration_type   = "AWS_PROXY"
  integration_uri    = aws_lambda_function.disconnect.invoke_arn
}

resource "aws_apigatewayv2_route" "default" {
  api_id    = aws_apigatewayv2_api.chat.id
  route_key = "$default"
  target    = "integrations/${aws_apigatewayv2_integration.default_handler.id}"
}

resource "aws_apigatewayv2_integration" "default_handler" {
  api_id             = aws_apigatewayv2_api.chat.id
  integration_type   = "AWS_PROXY"
  integration_uri    = aws_lambda_function.default_handler.invoke_arn
}

resource "aws_apigatewayv2_route" "sendmessage" {
  api_id    = aws_apigatewayv2_api.chat.id
  route_key = "sendmessage"
  target    = "integrations/${aws_apigatewayv2_integration.sendmessage.id}"
}

resource "aws_apigatewayv2_integration" "sendmessage" {
  api_id             = aws_apigatewayv2_api.chat.id
  integration_type   = "AWS_PROXY"
  integration_uri    = aws_lambda_function.sendmessage.invoke_arn
}

# --- IAM (required by Terraform, not used locally) ---

resource "aws_iam_role" "lambda" {
  name = "chat-lambda-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "lambda.amazonaws.com" }
    }]
  })
}
