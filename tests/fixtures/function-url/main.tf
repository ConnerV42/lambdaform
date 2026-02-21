resource "aws_lambda_function" "api" {
  function_name = "api-handler"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "lambda.zip"
}

resource "aws_lambda_function_url" "api_url" {
  function_name      = aws_lambda_function.api.function_name
  authorization_type = "NONE"

  cors {
    allow_origins     = ["https://example.com", "https://app.example.com"]
    allow_methods     = ["GET", "POST", "PUT", "DELETE"]
    allow_headers     = ["Content-Type", "Authorization"]
    expose_headers    = ["X-Request-Id"]
    max_age           = 3600
    allow_credentials = true
  }
}

resource "aws_lambda_function" "worker" {
  function_name = "worker-handler"
  handler       = "worker.handler"
  runtime       = "python3.12"
  filename      = "worker.zip"
}

resource "aws_lambda_function_url" "worker_url" {
  function_name      = aws_lambda_function.worker.function_name
  authorization_type = "AWS_IAM"
}
