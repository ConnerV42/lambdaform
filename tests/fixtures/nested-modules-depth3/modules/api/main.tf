module "v2" {
  source = "./modules/v2"
}

resource "aws_lambda_function" "list" {
  function_name = "api-list"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
}
