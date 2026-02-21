# From Zero to Serverless Locally: A Step-by-Step Terraform + Lambdaform Tutorial

*Build a working API with AWS Lambda and API Gateway — tested entirely on your machine before touching AWS.*

---

You've got Terraform. You've got Lambda functions. You want to test them locally before deploying. This tutorial walks you through building a small REST API from scratch using Terraform and Lambdaform, with zero AWS costs during development.

By the end, you'll have:
- A Terraform-defined REST API with three Lambda functions
- Local development with hot reload
- Request replay for testing
- A path to deploy the same code to real AWS

## Prerequisites

- **Node.js 20+** (we'll use Node.js, but Python/Go/Rust work too)
- **Terraform or OpenTofu** (for deployment later — not required for local dev)
- **Lambdaform** installed:

```bash
# macOS/Linux
brew tap ConnerV42/lambdaform
brew install lambdaform

# Or via npm
npx lambdaform --help

# Or via Cargo
cargo install lambdaform
```

Verify it works:

```bash
lambdaform --version
# lambdaform 1.0.1
```

## Step 1: Project Structure

Create a new project:

```bash
mkdir bookshelf-api && cd bookshelf-api
```

We'll build a tiny bookshelf API — list books, get a book, add a book. Simple enough to follow, complex enough to be realistic.

Create this structure:

```
bookshelf-api/
├── main.tf
├── variables.tf
├── terraform.tfvars
├── lambdaform.yaml        # optional config
└── src/
    ├── list-books.js
    ├── get-book.js
    └── add-book.js
```

## Step 2: Define Your Lambda Functions

**`src/list-books.js`** — Returns all books:

```javascript
const books = [
  { id: "1", title: "The Pragmatic Programmer", author: "Hunt & Thomas", year: 2019 },
  { id: "2", title: "Designing Data-Intensive Applications", author: "Martin Kleppmann", year: 2017 },
  { id: "3", title: "The Rust Programming Language", author: "Klabnik & Nichols", year: 2023 },
];

exports.handler = async (event) => {
  const topic = event.queryStringParameters?.topic;

  let filtered = books;
  if (topic) {
    filtered = books.filter(b =>
      b.title.toLowerCase().includes(topic.toLowerCase())
    );
  }

  return {
    statusCode: 200,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ books: filtered, count: filtered.length }),
  };
};
```

**`src/get-book.js`** — Returns a single book by ID:

```javascript
const books = [
  { id: "1", title: "The Pragmatic Programmer", author: "Hunt & Thomas", year: 2019 },
  { id: "2", title: "Designing Data-Intensive Applications", author: "Martin Kleppmann", year: 2017 },
  { id: "3", title: "The Rust Programming Language", author: "Klabnik & Nichols", year: 2023 },
];

exports.handler = async (event) => {
  const id = event.pathParameters?.id;
  const book = books.find(b => b.id === id);

  if (!book) {
    return {
      statusCode: 404,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ error: "Book not found" }),
    };
  }

  return {
    statusCode: 200,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(book),
  };
};
```

**`src/add-book.js`** — Accepts a new book (in-memory, for demo purposes):

```javascript
exports.handler = async (event) => {
  let body;
  try {
    body = JSON.parse(event.body || "{}");
  } catch {
    return {
      statusCode: 400,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ error: "Invalid JSON" }),
    };
  }

  if (!body.title || !body.author) {
    return {
      statusCode: 400,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ error: "title and author are required" }),
    };
  }

  const newBook = {
    id: String(Date.now()),
    title: body.title,
    author: body.author,
    year: body.year || new Date().getFullYear(),
  };

  return {
    statusCode: 201,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ message: "Book added", book: newBook }),
  };
};
```

## Step 3: Write the Terraform

**`variables.tf`**:

```hcl
variable "region" {
  default = "us-west-2"
}

variable "environment" {
  default = "dev"
}

variable "log_level" {
  default = "INFO"
}
```

**`terraform.tfvars`**:

```hcl
region      = "us-west-2"
environment = "dev"
log_level   = "DEBUG"
```

**`main.tf`**:

```hcl
provider "aws" {
  region = var.region
}

# --- IAM Role (used by all functions) ---

resource "aws_iam_role" "lambda_role" {
  name = "bookshelf-lambda-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "lambda.amazonaws.com" }
    }]
  })
}

# --- Lambda Functions ---

resource "aws_lambda_function" "list_books" {
  function_name = "bookshelf-list-books"
  handler       = "list-books.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda_role.arn
  filename      = "src/list-books.js"

  environment {
    variables = {
      ENVIRONMENT = var.environment
      LOG_LEVEL   = var.log_level
    }
  }
}

resource "aws_lambda_function" "get_book" {
  function_name = "bookshelf-get-book"
  handler       = "get-book.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda_role.arn
  filename      = "src/get-book.js"

  environment {
    variables = {
      ENVIRONMENT = var.environment
    }
  }
}

resource "aws_lambda_function" "add_book" {
  function_name = "bookshelf-add-book"
  handler       = "add-book.handler"
  runtime       = "nodejs20.x"
  role          = aws_iam_role.lambda_role.arn
  filename      = "src/add-book.js"

  environment {
    variables = {
      ENVIRONMENT = var.environment
    }
  }
}

# --- API Gateway (REST API v1) ---

resource "aws_api_gateway_rest_api" "bookshelf" {
  name        = "bookshelf-api"
  description = "Bookshelf REST API"
}

# GET /books
resource "aws_api_gateway_resource" "books" {
  rest_api_id = aws_api_gateway_rest_api.bookshelf.id
  parent_id   = aws_api_gateway_rest_api.bookshelf.root_resource_id
  path_part   = "books"
}

resource "aws_api_gateway_method" "list_books" {
  rest_api_id   = aws_api_gateway_rest_api.bookshelf.id
  resource_id   = aws_api_gateway_resource.books.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "list_books" {
  rest_api_id             = aws_api_gateway_rest_api.bookshelf.id
  resource_id             = aws_api_gateway_resource.books.id
  http_method             = aws_api_gateway_method.list_books.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = "arn:aws:apigateway:${var.region}:lambda:path/2015-03-31/functions/${aws_lambda_function.list_books.arn}/invocations"
}

# POST /books
resource "aws_api_gateway_method" "add_book" {
  rest_api_id   = aws_api_gateway_rest_api.bookshelf.id
  resource_id   = aws_api_gateway_resource.books.id
  http_method   = "POST"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "add_book" {
  rest_api_id             = aws_api_gateway_rest_api.bookshelf.id
  resource_id             = aws_api_gateway_resource.books.id
  http_method             = aws_api_gateway_method.add_book.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = "arn:aws:apigateway:${var.region}:lambda:path/2015-03-31/functions/${aws_lambda_function.add_book.arn}/invocations"
}

# GET /books/{id}
resource "aws_api_gateway_resource" "book" {
  rest_api_id = aws_api_gateway_rest_api.bookshelf.id
  parent_id   = aws_api_gateway_resource.books.id
  path_part   = "{id}"
}

resource "aws_api_gateway_method" "get_book" {
  rest_api_id   = aws_api_gateway_rest_api.bookshelf.id
  resource_id   = aws_api_gateway_resource.book.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "get_book" {
  rest_api_id             = aws_api_gateway_rest_api.bookshelf.id
  resource_id             = aws_api_gateway_resource.book.id
  http_method             = aws_api_gateway_method.get_book.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = "arn:aws:apigateway:${var.region}:lambda:path/2015-03-31/functions/${aws_lambda_function.get_book.arn}/invocations"
}
```

This is real, deployable Terraform. When you're ready to go to AWS, just `terraform apply`.

## Step 4: Start Lambdaform

```bash
# Validate first
lambdaform validate

# Expected output:
# 🔍 Validating Terraform in: ./
#    Found 3 .tf file(s)
#    Found 3 function(s), 1 gateway(s), 3 route(s)
# ✅ Validation passed!
```

Now start the server:

```bash
lambdaform start
```

You should see:

```
📦 Lambda Functions:
   • bookshelf-list-books (Nodejs20) → list-books.handler
   • bookshelf-get-book (Nodejs20) → get-book.handler
   • bookshelf-add-book (Nodejs20) → add-book.handler
🌐 API Gateway: bookshelf-api (REST)
   GET  /books      → bookshelf-list-books
   POST /books      → bookshelf-add-book
   GET  /books/{id} → bookshelf-get-book
🔥 Server running at http://localhost:3000
```

Lambdaform parsed your `.tf` files, resolved variables from `terraform.tfvars`, and wired everything up. No Docker. No deploy. Sub-second startup.

## Step 5: Test Your API

In another terminal:

```bash
# List all books
curl -s localhost:3000/books | jq .

# Get a specific book
curl -s localhost:3000/books/2 | jq .

# Search by topic
curl -s "localhost:3000/books?topic=rust" | jq .

# Add a book
curl -s -X POST localhost:3000/books \
  -H "Content-Type: application/json" \
  -d '{"title": "Zero to Production in Rust", "author": "Luca Palmieri", "year": 2022}' | jq .

# Test error handling
curl -s localhost:3000/books/999 | jq .
curl -s -X POST localhost:3000/books -d 'not json' | jq .
```

Every request runs through Lambdaform's API Gateway emulation and invokes your handler — the same code path as production.

## Step 6: Hot Reload

Leave the server running. Edit `src/list-books.js` — add a book to the array:

```javascript
{ id: "4", title: "Staff Engineer", author: "Will Larson", year: 2021 },
```

Save the file. Lambdaform detects the change instantly:

```
🔄 Detected change in src/list-books.js — reloading...
```

Hit the endpoint again:

```bash
curl -s localhost:3000/books | jq '.count'
# 4
```

No restart. No rebuild. Your change is live in milliseconds.

## Step 7: Request Replay

Lambdaform records every request. After running your curl commands above, try:

```bash
# List recorded requests
lambdaform replay --list

# Replay a specific request
lambdaform replay --index 1

# Replay all requests (great for regression testing)
lambdaform replay --all
```

This is useful when you're iterating on handler logic — change code, replay the same requests, compare output.

## Step 8: Explore More Features

**Invoke a function directly** (bypassing API Gateway):

```bash
lambdaform invoke bookshelf-list-books \
  --payload '{"queryStringParameters": {"topic": "data"}}'
```

**See your infrastructure graph:**

```bash
lambdaform graph
```

```
bookshelf-api (REST API)
├── GET /books → bookshelf-list-books [Nodejs20]
├── POST /books → bookshelf-add-book [Nodejs20]
└── GET /books/{id} → bookshelf-get-book [Nodejs20]
```

**Estimate costs from local usage:**

```bash
lambdaform cost
```

**Use the terminal UI for a live dashboard:**

```bash
lambdaform start --tui
```

## Step 9: Optional Config File

For projects where you want to customize behavior, create `lambdaform.yaml`:

```yaml
port: 3000
cors:
  allow_origins:
    - "http://localhost:5173"
  allow_methods:
    - GET
    - POST
watch: true
json_log: false
```

This is optional — Lambdaform works without it by reading your `.tf` files directly.

## Step 10: Deploy to AWS

When you're confident in your code, deploy the real thing:

```bash
terraform init
terraform plan
terraform apply
```

The same `.tf` files that Lambdaform read locally are now creating real AWS resources. No translation layer, no config drift.

## What's Different from Production?

A few things to keep in mind:

1. **State is in-memory** — Lambdaform doesn't emulate DynamoDB, S3, or other services. For those, pair it with [DynamoDB Local](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/DynamoDBLocal.html) or [ElasticMQ](https://github.com/softwaremill/elasticmq)
2. **No IAM enforcement** — permissions aren't checked locally. Your code runs with full access
3. **Native execution** — functions run as OS processes, not in the Lambda execution environment. This means faster iteration but slightly different behavior for things like `/tmp` limits or memory constraints

For most development workflows, these differences don't matter. You're testing business logic, not infrastructure behavior.

## Next Steps

- **Read the docs:** [connerv42.github.io/lambdaform](https://connerv42.github.io/lambdaform/)
- **Try with Python:** Change `runtime` to `python3.12` and `handler` to `handler.handler` — it just works
- **Add a DynamoDB table:** Define `aws_dynamodb_table` in your Terraform files, run DynamoDB Local alongside Lambdaform
- **Set up debugging:** Use `lambdaform start --debug-port 9229` to attach a Node.js debugger from VS Code
- **Check out the VS Code extension:** Function explorer, one-click invoke, live logs

---

*[Conner Verret](https://github.com/ConnerV42) builds [Lambdaform](https://github.com/ConnerV42/lambdaform) — a Terraform-native local Lambda development server. Star it on GitHub if it saves you time.*
