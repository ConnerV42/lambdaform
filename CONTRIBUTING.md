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
# Build (on resource-constrained machines)
CARGO_BUILD_JOBS=1 cargo build

# Run against a test fixture
cargo run -- serve --dir tests/fixtures/simple-node

# Validate Terraform parsing
cargo run -- validate --dir tests/fixtures/simple-node
```

## Pull Requests

- One feature/fix per PR
- Include test fixtures for new functionality
- Run `cargo clippy` and `cargo fmt` before submitting
- Update README.md if adding user-facing features

## Reporting Issues

- Include your Terraform configuration (sanitized)
- Include the Lambdaform version (`lambdaform --version`)
- Include the error output

## Architecture

- `parser.rs` — HCL/Terraform parsing
- `config.rs` — Configuration management
- `router.rs` — API Gateway route matching
- `runtime.rs` — Lambda runtime (Node.js, Python)
- `server.rs` — HTTP server
- `watcher.rs` — Hot reload file watcher

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
