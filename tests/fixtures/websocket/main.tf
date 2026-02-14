resource "aws_lambda_function" "connect" {
  function_name = "ws-connect"
  handler       = "index.connect"
  runtime       = "nodejs20.x"
}

resource "aws_lambda_function" "disconnect" {
  function_name = "ws-disconnect"
  handler       = "index.disconnect"
  runtime       = "nodejs20.x"
}

resource "aws_lambda_function" "default" {
  function_name = "ws-default"
  handler       = "index.default"
  runtime       = "nodejs20.x"
}

resource "aws_lambda_function" "sendmessage" {
  function_name = "ws-sendmessage"
  handler       = "index.sendmessage"
  runtime       = "nodejs20.x"
}

resource "aws_apigatewayv2_api" "ws" {
  name                       = "websocket-api"
  protocol_type              = "WEBSOCKET"
  route_selection_expression = "$request.body.action"
}

resource "aws_apigatewayv2_integration" "connect" {
  api_id           = aws_apigatewayv2_api.ws.id
  integration_type = "AWS_PROXY"
  integration_uri  = aws_lambda_function.connect.invoke_arn
}

resource "aws_apigatewayv2_integration" "disconnect" {
  api_id           = aws_apigatewayv2_api.ws.id
  integration_type = "AWS_PROXY"
  integration_uri  = aws_lambda_function.disconnect.invoke_arn
}

resource "aws_apigatewayv2_integration" "default" {
  api_id           = aws_apigatewayv2_api.ws.id
  integration_type = "AWS_PROXY"
  integration_uri  = aws_lambda_function.default.invoke_arn
}

resource "aws_apigatewayv2_integration" "sendmessage" {
  api_id           = aws_apigatewayv2_api.ws.id
  integration_type = "AWS_PROXY"
  integration_uri  = aws_lambda_function.sendmessage.invoke_arn
}

resource "aws_apigatewayv2_route" "connect" {
  api_id    = aws_apigatewayv2_api.ws.id
  route_key = "$connect"
  target    = aws_apigatewayv2_integration.connect.id
}

resource "aws_apigatewayv2_route" "disconnect" {
  api_id    = aws_apigatewayv2_api.ws.id
  route_key = "$disconnect"
  target    = aws_apigatewayv2_integration.disconnect.id
}

resource "aws_apigatewayv2_route" "default" {
  api_id    = aws_apigatewayv2_api.ws.id
  route_key = "$default"
  target    = aws_apigatewayv2_integration.default.id
}

resource "aws_apigatewayv2_route" "sendmessage" {
  api_id    = aws_apigatewayv2_api.ws.id
  route_key = "sendmessage"
  target    = aws_apigatewayv2_integration.sendmessage.id
}
