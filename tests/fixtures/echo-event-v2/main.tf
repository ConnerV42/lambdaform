resource "aws_lambda_function" "echo" {
  function_name = "echo-handler-v2"
  runtime       = "nodejs20.x"
  handler       = "index.handler"
  filename      = "lambda.zip"
  timeout       = 30
  memory_size   = 128
}

resource "aws_apigatewayv2_api" "http_api" {
  name          = "echo-http-api"
  protocol_type = "HTTP"
}

resource "aws_apigatewayv2_integration" "echo" {
  api_id             = aws_apigatewayv2_api.http_api.id
  integration_type   = "AWS_PROXY"
  integration_uri    = aws_lambda_function.echo.invoke_arn
  payload_format_version = "2.0"
}

resource "aws_apigatewayv2_route" "echo" {
  api_id    = aws_apigatewayv2_api.http_api.id
  route_key = "ANY /echo"
  target    = "integrations/${aws_apigatewayv2_integration.echo.id}"
}

resource "aws_apigatewayv2_route" "default" {
  api_id    = aws_apigatewayv2_api.http_api.id
  route_key = "$default"
  target    = "integrations/${aws_apigatewayv2_integration.echo.id}"
}
