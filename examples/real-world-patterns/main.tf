# Real-world Terraform patterns that stress-test the parser
# Tests: for_each, count, dynamic blocks, complex interpolation, templatefile, data sources

terraform {
  required_version = ">= 1.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

provider "aws" {
  region = var.aws_region
}

# --- Variables with defaults, validation, complex types ---

variable "aws_region" {
  type    = string
  default = "us-west-2"
}

variable "environment" {
  type    = string
  default = "dev"
  validation {
    condition     = contains(["dev", "staging", "prod"], var.environment)
    error_message = "Environment must be dev, staging, or prod."
  }
}

variable "api_name" {
  type    = string
  default = "real-world-api"
}

variable "functions" {
  type = map(object({
    handler     = string
    runtime     = string
    memory_size = number
    timeout     = number
    description = string
  }))
  default = {
    "list-items" = {
      handler     = "handlers/list.handler"
      runtime     = "nodejs20.x"
      memory_size = 256
      timeout     = 30
      description = "List all items with pagination"
    }
    "get-item" = {
      handler     = "handlers/get.handler"
      runtime     = "nodejs20.x"
      memory_size = 128
      timeout     = 10
      description = "Get a single item by ID"
    }
    "create-item" = {
      handler     = "handlers/create.handler"
      runtime     = "nodejs20.x"
      memory_size = 256
      timeout     = 30
      description = "Create a new item"
    }
  }
}

variable "cors_origins" {
  type    = list(string)
  default = ["https://example.com", "http://localhost:3001"]
}

variable "tags" {
  type = map(string)
  default = {
    Project     = "real-world-patterns"
    ManagedBy   = "terraform"
  }
}

# --- Locals with cross-references and complex expressions ---

locals {
  name_prefix    = "${var.api_name}-${var.environment}"
  log_group_name = "/aws/lambda/${local.name_prefix}"
  
  # Computed from variables
  cors_origin_string = join(",", var.cors_origins)
  
  common_env = {
    ENVIRONMENT  = var.environment
    REGION       = var.aws_region
    TABLE_NAME   = "${local.name_prefix}-items"
    CORS_ORIGINS = local.cors_origin_string
    LOG_LEVEL    = var.environment == "prod" ? "warn" : "debug"
  }

  # Merge tags
  all_tags = merge(var.tags, {
    Environment = var.environment
  })
}

# --- IAM Role (shared) ---

data "aws_iam_policy_document" "lambda_assume" {
  statement {
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "lambda" {
  name               = "${local.name_prefix}-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume.json
  tags               = local.all_tags
}

resource "aws_iam_role_policy_attachment" "lambda_basic" {
  role       = aws_iam_role.lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

# --- Lambda Functions (one per entry, NOT using for_each) ---

resource "aws_lambda_function" "list_items" {
  function_name = "${local.name_prefix}-list-items"
  role          = aws_iam_role.lambda.arn
  handler       = "handlers/list.handler"
  runtime       = "nodejs20.x"
  memory_size   = 256
  timeout       = 30
  filename      = "lambda.zip"

  environment {
    variables = merge(local.common_env, {
      FUNCTION_PURPOSE = "list"
      PAGE_SIZE        = "25"
    })
  }

  tags = local.all_tags
}

resource "aws_lambda_function" "get_item" {
  function_name = "${local.name_prefix}-get-item"
  role          = aws_iam_role.lambda.arn
  handler       = "handlers/get.handler"
  runtime       = "nodejs20.x"
  memory_size   = 128
  timeout       = 10
  filename      = "lambda.zip"

  environment {
    variables = merge(local.common_env, {
      FUNCTION_PURPOSE = "get"
    })
  }

  tags = local.all_tags
}

resource "aws_lambda_function" "create_item" {
  function_name = "${local.name_prefix}-create-item"
  role          = aws_iam_role.lambda.arn
  handler       = "handlers/create.handler"
  runtime       = "nodejs20.x"
  memory_size   = 256
  timeout       = 30
  filename      = "lambda.zip"

  environment {
    variables = merge(local.common_env, {
      FUNCTION_PURPOSE = "create"
    })
  }

  tags = local.all_tags
}

# --- API Gateway (REST v1) ---

resource "aws_api_gateway_rest_api" "api" {
  name        = local.name_prefix
  description = "Real-world patterns API (${var.environment})"
}

resource "aws_api_gateway_resource" "items" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "items"
}

resource "aws_api_gateway_resource" "item" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_resource.items.id
  path_part   = "{id}"
}

# GET /items → list
resource "aws_api_gateway_method" "list_items" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.items.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "list_items" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.items.id
  http_method             = aws_api_gateway_method.list_items.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.list_items.invoke_arn
}

# GET /items/{id} → get
resource "aws_api_gateway_method" "get_item" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.item.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "get_item" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.item.id
  http_method             = aws_api_gateway_method.get_item.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.get_item.invoke_arn
}

# POST /items → create
resource "aws_api_gateway_method" "create_item" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.items.id
  http_method   = "POST"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "create_item" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.items.id
  http_method             = aws_api_gateway_method.create_item.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.create_item.invoke_arn
}

# --- DynamoDB Table ---

resource "aws_dynamodb_table" "items" {
  name         = "${local.name_prefix}-items"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "pk"
  range_key    = "sk"

  attribute {
    name = "pk"
    type = "S"
  }

  attribute {
    name = "sk"
    type = "S"
  }

  attribute {
    name = "gsi1pk"
    type = "S"
  }

  global_secondary_index {
    name            = "gsi1"
    hash_key        = "gsi1pk"
    range_key       = "sk"
    projection_type = "ALL"
  }

  tags = local.all_tags
}
