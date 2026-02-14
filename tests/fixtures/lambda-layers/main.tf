resource "aws_lambda_layer_version" "utils" {
  layer_name          = "utils-layer"
  filename            = "layers/utils-layer"
  compatible_runtimes = ["nodejs20.x"]
}

resource "aws_lambda_function" "app" {
  function_name = "layer-test"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "."

  layers = [aws_lambda_layer_version.utils.arn]
}

resource "aws_api_gateway_rest_api" "api" {
  name = "layers-api"
}

resource "aws_api_gateway_resource" "test" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "test"
}

resource "aws_api_gateway_method" "get_test" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  resource_id = aws_api_gateway_resource.test.id
  http_method = "GET"
}

resource "aws_api_gateway_integration" "get_test" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  resource_id = aws_api_gateway_resource.test.id
  http_method = "GET"
  uri         = aws_lambda_function.app.invoke_arn
}
