resource "aws_lambda_function" "authorizer" {
  function_name = "my-authorizer"
  handler       = "auth.handler"
  runtime       = "nodejs20.x"
}

resource "aws_lambda_function" "protected" {
  function_name = "protected-api"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
}

resource "aws_lambda_function" "public" {
  function_name = "public-api"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
}

resource "aws_api_gateway_rest_api" "api" {
  name = "authorizer-test-api"
}

resource "aws_api_gateway_authorizer" "token_auth" {
  name            = "token-authorizer"
  rest_api_id     = aws_api_gateway_rest_api.api.id
  authorizer_uri  = aws_lambda_function.authorizer.invoke_arn
  type            = "TOKEN"
}

resource "aws_api_gateway_resource" "protected" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "protected"
}

resource "aws_api_gateway_resource" "public" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "public"
}

resource "aws_api_gateway_method" "protected_get" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.protected.id
  http_method   = "GET"
  authorization = "CUSTOM"
  authorizer_id = aws_api_gateway_authorizer.token_auth.id
}

resource "aws_api_gateway_method" "public_get" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.public.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "protected" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  resource_id = aws_api_gateway_resource.protected.id
  http_method = aws_api_gateway_method.protected_get.http_method
  type        = "AWS_PROXY"
  uri         = aws_lambda_function.protected.invoke_arn
}

resource "aws_api_gateway_integration" "public" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  resource_id = aws_api_gateway_resource.public.id
  http_method = aws_api_gateway_method.public_get.http_method
  type        = "AWS_PROXY"
  uri         = aws_lambda_function.public.invoke_arn
}
