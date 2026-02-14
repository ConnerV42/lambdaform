# Go Lambda test fixture

resource "aws_lambda_function" "go_hello" {
  function_name = "go-hello"
  handler       = "main"
  runtime       = "provided.al2023"
  filename      = "bootstrap.zip"

  environment {
    variables = {
      GREETING = "Hello from Go Lambda"
    }
  }
}

resource "aws_api_gateway_rest_api" "go_api" {
  name = "go-test-api"
}

resource "aws_api_gateway_resource" "go_hello" {
  rest_api_id = aws_api_gateway_rest_api.go_api.id
  parent_id   = aws_api_gateway_rest_api.go_api.root_resource_id
  path_part   = "hello"
}

resource "aws_api_gateway_method" "go_hello_get" {
  rest_api_id   = aws_api_gateway_rest_api.go_api.id
  resource_id   = aws_api_gateway_resource.go_hello.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "go_hello_get" {
  rest_api_id             = aws_api_gateway_rest_api.go_api.id
  resource_id             = aws_api_gateway_resource.go_hello.id
  http_method             = aws_api_gateway_method.go_hello_get.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.go_hello.invoke_arn
}

resource "aws_api_gateway_resource" "go_greet" {
  rest_api_id = aws_api_gateway_rest_api.go_api.id
  parent_id   = aws_api_gateway_rest_api.go_api.root_resource_id
  path_part   = "greet"
}

resource "aws_api_gateway_resource" "go_greet_name" {
  rest_api_id = aws_api_gateway_rest_api.go_api.id
  parent_id   = aws_api_gateway_resource.go_greet.id
  path_part   = "{name}"
}

resource "aws_api_gateway_method" "go_greet_get" {
  rest_api_id   = aws_api_gateway_rest_api.go_api.id
  resource_id   = aws_api_gateway_resource.go_greet_name.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "go_greet_get" {
  rest_api_id             = aws_api_gateway_rest_api.go_api.id
  resource_id             = aws_api_gateway_resource.go_greet_name.id
  http_method             = aws_api_gateway_method.go_greet_get.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.go_hello.invoke_arn
}
