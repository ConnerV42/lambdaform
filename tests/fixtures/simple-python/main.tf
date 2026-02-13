# Python Lambda functions for testing Lambdaform

resource "aws_lambda_function" "py_hello" {
  function_name = "py-hello-world"
  handler       = "handler.handler"
  runtime       = "python3.12"
  timeout       = 30
  memory_size   = 128

  filename = "lambda.zip"

  environment {
    variables = {
      GREETING = "Howdy from Python Lambdaform!"
      ENV      = "local"
    }
  }
}

resource "aws_api_gateway_rest_api" "api" {
  name        = "python-api"
  description = "Python API for testing"
}

resource "aws_api_gateway_resource" "hello" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "hello"
}

resource "aws_api_gateway_method" "get_hello" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.hello.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "get_hello" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.hello.id
  http_method             = aws_api_gateway_method.get_hello.http_method
  type                    = "AWS_PROXY"
  integration_http_method = "POST"
  uri                     = aws_lambda_function.py_hello.invoke_arn
}

resource "aws_lambda_function" "py_echo" {
  function_name = "py-echo"
  handler       = "echo.handler"
  runtime       = "python3.12"
  timeout       = 10
  memory_size   = 128

  filename = "lambda.zip"
}

resource "aws_api_gateway_resource" "echo" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "echo"
}

resource "aws_api_gateway_method" "post_echo" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.echo.id
  http_method   = "POST"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "post_echo" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.echo.id
  http_method             = aws_api_gateway_method.post_echo.http_method
  type                    = "AWS_PROXY"
  integration_http_method = "POST"
  uri                     = aws_lambda_function.py_echo.invoke_arn
}
