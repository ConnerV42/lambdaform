# Contributing to Lambdaform

Thanks for your interest in contributing! 🚀

## Getting Started

1. Fork the repo
2. Clone your fork
3. Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
4. Build: `cargo build`
5. Run tests: `cargo test`

## Development

```bash
# Build
cargo build

# Build with Terminal UI feature
cargo build --features tui

# Run against a test fixture
cargo run -- start --dir tests/fixtures/simple-node

# Validate Terraform parsing
cargo run -- validate --dir tests/fixtures/simple-node

# Run a specific test
cargo test test_name

# Lint
cargo clippy -- -D warnings
cargo fmt --check
```

## Before Submitting

1. Run `cargo fmt --all` — formatting is enforced in CI
2. Run `cargo clippy -- -D warnings` — zero warnings policy
3. Run `cargo test` — all tests must pass
4. Update docs if adding user-facing features (see `docs/src/`)

## Pull Requests

- One feature/fix per PR
- Include tests for new functionality (unit tests in the module, integration tests in `tests/`)
- Add test fixtures in `tests/fixtures/` for new Terraform resource types
- Update `CHANGELOG.md` with your changes
- Update `README.md` if adding user-facing features
- Update `docs/src/` for documentation site changes

## Reporting Issues

- Include your Terraform/OpenTofu configuration (sanitized)
- Include the Lambdaform version (`lambdaform --version`)
- Include the full error output
- Mention your OS and architecture (macOS ARM64, Linux x86_64, etc.)

## Architecture

The codebase is in `src/` with each module handling a distinct concern:

| Module | Purpose |
|--------|---------|
| `parser.rs` | HCL/Terraform parsing — extracts Lambda, API Gateway, DynamoDB, SQS, SNS, and Step Functions resources |
| `config.rs` | Configuration types and defaults |
| `project_config.rs` | `lambdaform.yaml` config file loading and per-function overrides |
| `router.rs` | API Gateway route matching with path parameter extraction |
| `runtime.rs` | Lambda runtime invocation (Node.js, Python, Go, Rust, Java/Docker) |
| `server.rs` | HTTP server — builds Lambda events from HTTP requests, manages API Gateways |
| `pool.rs` | Warm process pooling for fast invocations |
| `websocket.rs` | WebSocket API Gateway support ($connect/$disconnect/$default, @connections) |
| `trigger.rs` | SQS/SNS trigger simulation with realistic event payloads |
| `stepfunctions.rs` | Step Functions state machine visualization and local execution |
| `graph.rs` | Infrastructure graph visualization (ASCII/DOT/JSON) |
| `cost.rs` | Cost estimation from request history |
| `history.rs` | Request recording and replay (JSONL) |
| `watcher.rs` | Hot reload via file system watching |
| `plugin.rs` | Plugin architecture for custom resource handlers |
| `tui.rs` | Terminal UI dashboard (optional `tui` feature) |
| `main.rs` | CLI entry point (clap) |

Integration tests live in `tests/integration.rs` with fixtures in `tests/fixtures/`.

## Test Fixtures

Each fixture in `tests/fixtures/` is a minimal Terraform project. When adding support for a new resource type:

1. Create a new fixture directory: `tests/fixtures/my-feature/`
2. Add a `main.tf` with the relevant resources
3. Add a handler file if needed (e.g., `index.js`, `handler.py`)
4. Write unit tests in the relevant module
5. Write integration tests in `tests/integration.rs`

## Documentation Site

The docs site uses [mdBook](https://rust-lang.github.io/mdBook/). Source is in `docs/src/`.

```bash
# Install mdBook
cargo install mdbook

# Serve docs locally
cd docs && mdbook serve
```

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
