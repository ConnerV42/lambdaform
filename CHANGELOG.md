# Changelog

## v1.0.2 — Polish & Test Coverage (2026-02-22)

**Features:**
- `lambdaform graph --port <PORT>` — show port assignments in infrastructure graph

**Documentation:**
- Added 5 missing CLI commands to README (init, replay, cost, graph, plugins)
- All 13 subcommands now documented with usage examples

**Test coverage:**
- 40 new tests across 5 modules (router, websocket, trigger, history, project_config)
- Total tests: 204 (up from 164)

## v1.0.1 — Pre-Launch QA (2026-02-20)

Comprehensive QA sprint: 37 issues identified, 33 fixed, 4 deferred to post-launch.

**Bug fixes (Critical — all fixed):**
- Fix process leak on timeout (child processes now killed after timeout)
- Fix pool worker deadlock under concurrent load (per-worker locking)
- Fix UUID collisions under concurrent requests (switched to UUIDv4)
- Fix RIE server leak on error paths (cleanup on spawn failure)

**Bug fixes (High):**
- Helpful errors for missing runtime binaries (node/python3 not found)
- Fix hardcoded date in trigger timestamps (now uses real chrono timestamps)
- Forward stderr in non-pool mode (console.log/print no longer silently dropped)
- Fix WebSocket @connections port overflow (configurable, bounds-checked)
- Complete API Gateway v2 event format (cookies, stageVariables)
- Support `jsonencode()` + 12 Terraform functions in parser
- Fix Python worker hang on import failure (startup handshake)
- Improve source_path resolution for zip-based deploys
- Handle `count`/`for_each` in parser (warns and continues)

**Bug fixes (Medium):**
- Add `multiValueQueryStringParameters` and `multiValueHeaders` to v1 events
- Base64-encode binary request bodies in Lambda events
- Remove `deny_unknown_fields` from ProjectConfig (forward compatibility)
- Add 10MB request body size limit (matches AWS)
- History file rotation (keep last 1000 entries)
- Support `.tfvars.json` file format
- Fix Go build target (builds for host platform, not hardcoded linux)
- Fix nested module prefix at depth >2 (recursive prefixing)
- Validate handler function exists at startup (warns if handler file missing)
- Fix Step Functions `jsonencode` resolution

**New features:**
- `nodejs22.x` and `python3.13` runtime support
- Trigger `--dry-run` flag
- Config file hot-reload (watches `lambdaform.yaml` for changes)

**Testing:**
- Expanded from 125 to 162 tests (120 unit + 42 integration)
- Added watcher tests (config detection, source files, tfvars)
- Added parser tests (depth-3 modules, count/for_each, .tfvars.json)

## v1.0.0 — Ready for Teams (2026-02-16)

🎉 **First stable release!** Lambdaform is production-ready for team adoption.

**New features:**
- Comprehensive documentation site (mdBook, 30+ pages, GitHub Pages deployment)
- Rust runtime support (`provided.al2023` with auto-detect, cargo build integration, smart rebuild)
- Java/JVM runtime support (Docker-based, `java8.al2`/`java11`/`java17`/`java21` via AWS Lambda base images)
- VS Code extension (function explorer, one-click invoke, live log viewer, server control, status bar)
- CI/CD integration guide (5 GitHub Actions patterns, 3 GitLab CI configs, test examples)
- Plugin architecture (`lambdaform plugins` CLI, custom resource handlers, example S3 plugin)
- Cost estimation (`lambdaform cost` — per-function breakdown, monthly projection, free tier, ARM pricing)
- Infrastructure graph visualization (`lambdaform graph` — ASCII/DOT/JSON, detects all resource relationships)

**Improvements:**
- 125 tests passing (unit + integration)
- Clippy-clean, zero warnings
- 6 supported runtimes: Node.js, Python, Go, Rust, Java, custom

**Supported runtimes:**
| Runtime | Invocation |
|---------|-----------|
| Node.js 18.x / 20.x | Warm pool (~3ms) |
| Python 3.10–3.12 | Warm pool (~3ms) |
| Go 1.x / provided.al2/al2023 | Mini RIE (~14ms) |
| Rust (provided.al2023) | Mini RIE (~14ms) |
| Java 8/11/17/21 | Docker (~500ms) |

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
