# Changelog

## v0.6.0 — Developer Experience (2026-02-16)

**New features:**
- Terraform variable resolution from `.tfvars` files (`--var-file` CLI flag)
- Terraform module support (local modules)
- Better error messages with source location (file:line:col for parse errors)
- `lambdaform init` — guided project setup with structure detection and `--yes` flag
- `lambdaform replay` — request history recording to JSONL, replay via native HTTP client, filtering
- Structured JSON logging mode (`--json-log` flag / `json_log` config)
- Environment variable and locals interpolation (`${local.xxx}`, `${var.name}`)
- Terminal UI — optional ratatui dashboard with live request log, color-coded methods/status/timing, keyboard navigation (feature-gated)

**Bug fixes:**
- Fixed replay curl dependency — replaced shell-out with native hyper HTTP client
- Fixed 4 bugs found during Civic Scanner v2 dogfooding

## v0.5.0 — Production Hardening (2026-02-15)

**Bug fixes:**
- Fix Lambda timeout enforcement (processes now killed after configured timeout)
- Fix nested API Gateway v1 path resolution
- Fix WebSocket event format (now sends proper WebSocketEvent)
- Fix base64 encoding in WebSocket module (was hex)
- Replace 18 non-test `unwrap()` calls with proper error handling

**Quality & robustness:**
- Integration test suite (14 tests covering REST API Gateway scenarios)
- Graceful shutdown (SIGINT handler, worker pool cleanup, listener close)

**Distribution:**
- Cross-platform CI (macOS x86_64 + ARM64 test jobs)
- Homebrew formula (`brew install ConnerV42/tap/lambdaform`)
- npm wrapper (`npx lambdaform`)
- OpenTofu compatibility (tested + unit test)

**Documentation:**
- Asciinema terminal demo embedded in README
- Launch blog post: "Why Local Lambda Dev with Terraform Is Broken"

## v0.4.0 — Lambda Ecosystem (2026-02-14)

- Lambda layers support
- WebSocket API Gateway
- Step Functions visualization
- DynamoDB Local integration hints
- SQS/SNS trigger simulation

## v0.3.0 — Developer Tooling (2026-02-13)

- Go runtime support
- Node.js + Python debugger integration
- Multiple API Gateway support
- Warm process pooling

## v0.2.0 — API Gateway + DX (2026-02-13)

- HTTP API Gateway v2
- Lambda authorizers (v1 + v2)
- Config file (`lambdaform.yaml`)
- CORS handling
- Request logging improvements
- Real AWS validation (2 bugs found and fixed)

## v0.1.0 — MVP (2026-02-13)

- Parse `aws_lambda_function` from HCL
- Node.js + Python runtimes
- API Gateway REST routing
- Hot reload
- No Docker required
