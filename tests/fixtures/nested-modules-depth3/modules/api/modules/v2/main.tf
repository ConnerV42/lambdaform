resource "aws_lambda_function" "create" {
  function_name = "v2-create"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
}
