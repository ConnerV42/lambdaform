# Dogfooding Your Dev Tools: What I Learned Building a Real App with Lambdaform

*Five bugs, three design insights, and one changed opinion — from using my own tool on a production project.*

---

I built [Lambdaform](https://github.com/ConnerV42/lambdaform) to solve my own problem: local Lambda development with Terraform. But building a tool and *using* a tool are different things. So I took my other project — Civic Scanner, a serverless app that analyzes local government meeting agendas — and rebuilt it from scratch using Lambdaform for all local development.

Here's what I found.

## The Project: Civic Scanner

Civic Scanner is a three-Lambda serverless app:

- **API Lambda** — FastAPI (Python) serving a React frontend, backed by DynamoDB
- **Ingestor Lambda** — scrapes city council meeting agendas from a municipal website, stores PDFs in S3
- **Analyzer Lambda** — sends meeting items to Claude (via AWS Bedrock) for AI analysis, writes results to DynamoDB

The Terraform defines API Gateway v2 (HTTP API), three Lambda functions, DynamoDB tables, S3 buckets, IAM roles, and an EventBridge rule for weekly ingestion. A real project, not a toy.

## Bug #1: Variable Resolution Ordering

**The problem:** My Terraform used `${var.region}` inside a `locals` block, and `${local.table_name}` in the Lambda environment variables. Lambdaform resolved variables but didn't resolve locals that *depended* on variables.

```hcl
locals {
  table_prefix = "${var.environment}-civic"
  table_name   = "${local.table_prefix}-meetings"
}

resource "aws_lambda_function" "api" {
  environment {
    variables = {
      TABLE_NAME = local.table_name  # Resolved to empty string
    }
  }
}
```

**The fix:** Lambdaform now does iterative resolution — it resolves variables first, then makes multiple passes over locals until all cross-references are resolved. This was a fundamental improvement to the parser.

**Lesson:** You can't discover resolution ordering bugs with simple test fixtures. You need real projects with realistic variable chains.

## Bug #2: Python Source Path for Zip Deploys

**The problem:** Civic Scanner packages each Lambda as a zip file with dependencies:

```hcl
resource "aws_lambda_function" "api" {
  filename         = "${path.module}/dist/api.zip"
  source_code_hash = filebase64sha256("${path.module}/dist/api.zip")
  handler          = "api.main.handler"
}
```

Lambdaform saw the `.zip` filename and couldn't find the handler. It was looking for `api.zip/api/main.py` instead of `src/api/main.py`.

**The fix:** Added `source_path` configuration in `lambdaform.yaml` so you can tell Lambdaform where your actual source lives, separate from where Terraform packages it:

```yaml
functions:
  civic-api:
    source_path: ./src/api
```

**Lesson:** Production Terraform often has a build/package step between source code and what gets deployed. Any local dev tool needs to account for this gap.

## Bug #3: Python Import Failures Hung Forever

**The problem:** My API Lambda imported `mangum`, `boto3`, and `anthropic`. When I first started Lambdaform, I hadn't installed these dependencies locally. The Python process started, tried to import, failed — and then *hung*. No error message, no timeout. Just... nothing.

**The fix:** Added a startup handshake protocol. When Lambdaform spawns a Python worker, it waits for a readiness signal. If the signal doesn't arrive within 10 seconds, it kills the process and prints the captured stderr — which includes the `ModuleNotFoundError`.

**Lesson:** The "happy path" is easy to test. The failure path is where users actually get stuck. A missing dependency is probably the #1 thing that happens when someone first tries a Lambda project locally.

## Bug #4: API Gateway v2 Event Format Was Incomplete

**The problem:** Civic Scanner uses API Gateway v2 (HTTP API), not v1 (REST API). The v2 event format is different — `requestContext` has different fields, cookies are a top-level array, `rawPath` and `rawQueryString` exist, and `multiValueHeaders` doesn't.

I'd implemented v2 support, but my implementation was based on the documentation. The *real* events AWS sends include fields the docs don't emphasize — like `stageVariables`, `isBase64Encoded` on the request (not just response), and specific `requestContext.http` subfields.

**The fix:** Compared Lambdaform's generated events against real CloudWatch logs from production. Fixed ~15 fields to match actual AWS behavior.

**Lesson:** AWS documentation is necessary but not sufficient. Test against real events.

## Bug #5: Worker Pool Stderr Buffering

**The problem:** With warm process pooling enabled (Lambdaform reuses Lambda processes for faster invocations), Python's `print()` statements were being silently dropped. The stderr pipe was filling up because nobody was reading it, and eventually the worker would hang.

**The fix:** Added background stderr draining for all pool workers. stderr output now streams to the Lambdaform console in real-time.

**Lesson:** Process pooling introduces IPC complexity that doesn't exist in one-shot execution. If you're keeping processes alive, you need to keep *all* their I/O flowing.

## Three Design Insights

### 1. The Config File Gap

I originally designed Lambdaform to work with *zero configuration* — just read `.tf` files and go. Civic Scanner taught me this is too idealistic. Real projects need:

- Source path overrides (zip deploys vs source directories)
- Custom ports (when running alongside DynamoDB Local)
- CORS configuration (frontend dev server on a different port)
- Function-specific environment variable overrides (local DynamoDB endpoint vs production)

The `lambdaform.yaml` config file started as an afterthought and became essential. Zero-config should be the default, but escape hatches matter.

### 2. Error Messages Are Features

When Lambdaform couldn't find `python3` on PATH, it originally printed:

```
Error: Failed to spawn process
```

After dogfooding, it now prints:

```
Error: Runtime 'python3.12' requires python3 on PATH
  → Install: https://www.python.org/downloads/
  → Or use pyenv: pyenv install 3.12
  → Detected python versions: python3.11 at /usr/bin/python3.11
```

Every error message I improved came from actually hitting that error during real development. You can't write good error messages from imagination.

### 3. The Multi-Tool Reality

Lambdaform emulates Lambda + API Gateway. It doesn't emulate DynamoDB, S3, SQS, or Bedrock. For Civic Scanner, my local dev setup was:

- **Lambdaform** — Lambda + API Gateway
- **DynamoDB Local** — database (Docker container)
- **LocalStack S3** — PDF storage (or just mock it)
- **Real Bedrock** — AI analysis (no good local emulator)

This is fine. Each tool does one thing well. But it means Lambdaform's documentation needs to guide users toward this multi-tool setup rather than pretending it's a complete solution.

## One Changed Opinion

Before dogfooding, I believed **Docker-free was always better.** After all, Lambdaform's selling point is "no Docker required."

I still believe that for iteration speed. But running Civic Scanner locally, I noticed subtle behavior differences between native Python execution and the Lambda runtime. Python's `tempfile` module creates files in different locations. Memory limits aren't enforced. The Lambda execution context (like `context.get_remaining_time_in_millis()`) returns mock values.

For most development, this doesn't matter. You're testing business logic, not infrastructure behavior. But for pre-production validation, you might want Docker anyway — and that's okay. Lambdaform is for the inner development loop. Integration testing might need a heavier tool.

## Was It Worth It?

Absolutely. Dogfooding found 5 bugs that no amount of unit testing would have caught. It improved error messages, drove the config file design, and gave me a realistic sense of what the tool feels like for a new user.

If you're building developer tools: use them. Not in a demo project with three files. In something messy and real, with dependencies and build steps and things that break. That's where the truth lives.

---

*[Conner Verret](https://github.com/ConnerV42) builds [Lambdaform](https://github.com/ConnerV42/lambdaform) — a Terraform-native local Lambda development server. [Civic Scanner](https://brief.connerv.com) is live if you want to see what Spokane's city council is discussing.*
