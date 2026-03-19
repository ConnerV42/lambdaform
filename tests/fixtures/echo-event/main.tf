resource "aws_lambda_function" "echo" {
  function_name = "echo-handler"
  runtime       = "nodejs20.x"
  handler       = "index.handler"
  filename      = "lambda.zip"
  timeout       = 30
  memory_size   = 128

  environment {
    variables = {
      ENV = "test"
    }
  }
}

# REST API v1
resource "aws_api_gateway_rest_api" "api" {
  name = "echo-api"
}

resource "aws_api_gateway_resource" "echo" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "echo"
}

resource "aws_api_gateway_method" "echo_any" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.echo.id
  http_method   = "ANY"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "echo_any" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.echo.id
  http_method             = aws_api_gateway_method.echo_any.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.echo.invoke_arn
}

resource "aws_api_gateway_resource" "echo_proxy" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_resource.echo.id
  path_part   = "{proxy+}"
}

resource "aws_api_gateway_method" "echo_proxy_any" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.echo_proxy.id
  http_method   = "ANY"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "echo_proxy_any" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.echo_proxy.id
  http_method             = aws_api_gateway_method.echo_proxy_any.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.echo.invoke_arn
}
