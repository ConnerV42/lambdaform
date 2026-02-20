# Test fixture for count and for_each meta-arguments

resource "aws_lambda_function" "worker" {
  count         = 3
  function_name = "worker-${count.index}"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "worker.zip"
}

resource "aws_lambda_function" "service" {
  for_each      = toset(["api", "auth", "notify"])
  function_name = "service-${each.key}"
  handler       = "index.handler"
  runtime       = "python3.12"
  filename      = "service.zip"
}

resource "aws_lambda_function" "singleton" {
  function_name = "singleton-handler"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "singleton.zip"
}
