# Installation

## Homebrew (macOS & Linux)

```bash
brew tap ConnerV42/lambdaform
brew install lambdaform
```

Updates:
```bash
brew upgrade lambdaform
```

## Cargo (from source)

Requires Rust 1.70+:

```bash
cargo install lambdaform
```

## npm / npx

Run without installing:
```bash
npx lambdaform start
```

Or install globally:
```bash
npm install -g lambdaform
```

The npm package downloads the appropriate native binary for your platform.

## Pre-built Binaries

Download from [GitHub Releases](https://github.com/ConnerV42/lambdaform/releases):

| Platform | Architecture | Binary |
|----------|-------------|--------|
| macOS | ARM64 (Apple Silicon) | `lambdaform-aarch64-apple-darwin` |
| macOS | x86_64 (Intel) | `lambdaform-x86_64-apple-darwin` |
| Linux | x86_64 | `lambdaform-x86_64-unknown-linux-gnu` |
| Linux | ARM64 | `lambdaform-aarch64-unknown-linux-gnu` |

```bash
# Example: Linux x86_64
curl -L https://github.com/ConnerV42/lambdaform/releases/latest/download/lambdaform-x86_64-unknown-linux-gnu -o lambdaform
chmod +x lambdaform
sudo mv lambdaform /usr/local/bin/
```

## Verify Installation

```bash
lambdaform --version
lambdaform --help
```

## Prerequisites

- **Node.js** (for Node.js Lambda functions) — 18.x or 20.x recommended
- **Python** (for Python Lambda functions) — 3.10, 3.11, or 3.12
- **Go** (for Go Lambda functions) — 1.x with compiled binary
- **Terraform or OpenTofu** — not required at runtime, but your `.tf` files must exist
