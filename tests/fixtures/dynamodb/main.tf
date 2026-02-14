resource "aws_dynamodb_table" "users" {
  name         = "users-table"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "userId"
  range_key    = "sortKey"

  attribute {
    name = "userId"
    type = "S"
  }

  attribute {
    name = "sortKey"
    type = "S"
  }

  attribute {
    name = "email"
    type = "S"
  }

  global_secondary_index {
    name            = "email-index"
    hash_key        = "email"
    projection_type = "ALL"
  }

  stream_enabled   = true
  stream_view_type = "NEW_AND_OLD_IMAGES"
}

resource "aws_dynamodb_table" "sessions" {
  name         = "sessions-table"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "sessionId"

  attribute {
    name = "sessionId"
    type = "S"
  }

  ttl {
    attribute_name = "expiresAt"
    enabled        = true
  }
}

resource "aws_lambda_function" "api" {
  function_name = "api-handler"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "lambda.zip"

  environment {
    variables = {
      USERS_TABLE    = aws_dynamodb_table.users.name
      SESSIONS_TABLE = aws_dynamodb_table.sessions.name
    }
  }
}
