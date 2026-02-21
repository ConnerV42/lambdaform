# Building a Lambda Emulator in Rust: Parsing HCL Without the HCL Library

*How Lambdaform reads Terraform files natively — and why we didn't use an HCL parser.*

---

If you want to build a tool that understands Terraform configurations, the obvious approach is to use HashiCorp's HCL library. There's just one problem: it's written in Go. And we're writing Rust.

This is the story of how [Lambdaform](https://github.com/ConnerV42/lambdaform) — a local Lambda development server — parses `.tf` files using a hand-rolled parser in Rust, and the tradeoffs that come with that decision.

## Why Parse .tf Files At All?

Lambdaform's core value proposition is simple: point it at your Terraform project, and it spins up a local development server. No `terraform apply`, no Docker, no YAML translation layer. Your `.tf` files *are* the configuration.

That means we need to extract:

- `aws_lambda_function` resources (handler, runtime, environment variables, layers)
- `aws_api_gateway_rest_api` and route definitions
- `aws_apigatewayv2_api` (HTTP APIs, WebSocket APIs)
- Lambda authorizers, SQS triggers, Step Functions definitions
- Variables, locals, module references, `.tfvars` files

We don't need to *plan* or *apply* anything. We just need to understand the structure.

## The "Just Use HCL" Problem

HashiCorp's `hcl/v2` Go library is the gold standard for parsing HCL. But embedding Go in a Rust binary isn't practical:

- **CGo FFI** adds complexity, cross-compilation headaches, and a Go runtime dependency
- **Shelling out** to a Go binary means distributing two binaries and dealing with IPC
- **WASM compilation** of the Go HCL library would work in theory, but the binary size and performance overhead aren't worth it

There *are* Rust HCL crates (`hcl-rs`), but at the time of writing, they handle HCL syntax without understanding Terraform semantics — variable interpolation, module resolution, `count`/`for_each`, built-in functions. We'd still be writing most of the logic ourselves.

So we wrote a purpose-built parser. Not a general HCL parser — a Terraform resource extractor.

## The Parser Architecture

The key insight: **we don't need to parse all of HCL**. We need to parse the subset that Terraform users actually write for Lambda infrastructure.

```
.tf files → Tokenizer → Block Parser → Resource Extractor → Project Model
                                              ↓
                                    Variable Resolver
                                    (tfvars, locals, defaults)
```

### Stage 1: Block Extraction

HCL is fundamentally a block-based language. Every meaningful construct looks like:

```hcl
resource "aws_lambda_function" "my_func" {
  filename      = "lambda.zip"
  function_name = "my-function"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
}
```

The first pass identifies block boundaries by tracking brace depth. This is simpler than it sounds — HCL doesn't have string-interpolated braces in block structures, so counting `{` and `}` (outside strings and comments) reliably finds block boundaries.

```rust
// Simplified block extraction
fn extract_blocks(content: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut depth = 0;
    let mut block_start = None;
    
    for (i, ch) in content.char_indices() {
        match ch {
            '{' => {
                if depth == 0 {
                    block_start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(start) = block_start {
                        blocks.push(parse_block(&content[..=i], start));
                    }
                }
            }
            _ => {}
        }
    }
    blocks
}
```

The real implementation handles strings (including heredocs), comments (`//`, `#`, `/* */`), and tracks source locations for error reporting.

### Stage 2: Attribute Extraction

Inside a block, we need key-value pairs. HCL uses `=` for assignment:

```hcl
handler       = "index.handler"
runtime       = "nodejs20.x"
timeout       = 30
memory_size   = 256
```

But also nested blocks (no `=`):

```hcl
environment {
  variables = {
    TABLE_NAME = "my-table"
    STAGE      = "dev"
  }
}
```

The parser distinguishes these by checking whether a line contains `=` at the top level (not inside a nested structure). Nested blocks are parsed recursively.

### Stage 3: Expression Resolution

This is where it gets interesting. Terraform values aren't always literals:

```hcl
function_name = var.prefix != "" ? "${var.prefix}-${var.function_name}" : var.function_name
handler       = "${var.handler_module}.handler"
timeout       = var.timeout
runtime       = local.lambda_runtime

environment {
  variables = {
    TABLE_NAME = aws_dynamodb_table.main.name  # We can't resolve this
    API_URL    = var.api_url                     # We can resolve this
  }
}
```

Lambdaform's `VariableResolver` handles:

- **`var.xxx`** → looks up in `.tfvars` files, variable defaults, or `--var` CLI flags
- **`local.xxx`** → resolves from `locals {}` blocks, with iterative resolution for cross-references
- **`"${interpolation}"`** → string template expansion
- **Terraform functions** → `jsonencode()`, `lookup()`, `coalesce()`, `lower()`, `replace()`, `format()`, `trimprefix()`, `trimsuffix()`, `join()`, `split()`, `length()`, `try()`, `tostring()`
- **Ternary expressions** → `condition ? true_val : false_val`
- **Resource references** → `aws_dynamodb_table.main.name` — returned as-is (we can't resolve these without state)

The function list grew organically from real-world Terraform projects. Every time someone's `.tf` files used a function we didn't support, we added it.

### Stage 4: Module Resolution

Terraform modules are directories of `.tf` files referenced from a parent:

```hcl
module "api" {
  source = "./modules/api"
  
  function_name = "my-api"
  runtime       = "python3.12"
}
```

Lambdaform follows `source` paths (local modules only), passes variables through as if they were `.tfvars`, and prefixes resource names to avoid collisions: `api__my_function` instead of `my_function`.

This works recursively — modules within modules work fine up to arbitrary depth.

## What We Don't Parse (And Why That's OK)

Being honest about limitations matters more than claiming completeness:

**`for` expressions:** `[for s in var.subnets : s.id]` — these require runtime evaluation. We skip them and use the raw string.

**`dynamic` blocks:** These generate blocks programmatically. We'd need a mini-interpreter. Instead, we warn and skip.

**`count` and `for_each` meta-arguments:** We parse them and warn that multiple instances won't be expanded. For local dev, having one instance of a function is usually enough.

**Complex conditionals:** Nested ternaries with function calls work in simple cases but can break on deeply nested expressions.

**Data sources and remote state:** `data.aws_region.current.name` can't be resolved without AWS credentials. We return the reference string.

The philosophy: **parse what we can, warn on what we can't, never crash on valid Terraform**. A user should be able to point Lambdaform at any Terraform project and get *something* useful, even if some resources can't be fully resolved.

## Error Reporting

When parsing fails, the error message needs to help:

```
Error: Failed to parse attribute value
  --> modules/api/main.tf:47:23
   |
47 |   handler = complex(expression, that, we, cant, parse)
   |                     ^ unexpected token
   |
   = help: Lambdaform supports a subset of HCL expressions.
           Try simplifying this expression or using a variable.
```

Every parse error includes file path, line number, column, and a help message. This was one of the later additions but made the biggest difference in user experience.

## Performance

The parser is fast. Embarrassingly fast, actually:

- **Small project (5 .tf files):** <5ms to parse
- **Medium project (20 .tf files, 3 modules):** ~15ms
- **Large project (50+ .tf files, nested modules):** ~40ms

This is the Rust advantage — zero-cost abstractions, no GC pauses, efficient string handling. Parse time is never the bottleneck; the server startup (binding ports, initializing worker pools) takes longer.

## Testing Strategy

The parser has the most tests of any module — currently 45+ parser-specific tests covering:

- Simple resources (one function, one API gateway)
- Multi-gateway projects
- Variable resolution from defaults, `.tfvars`, `.tfvars.json`
- Local cross-references (`local.a` references `local.b`)
- Module nesting (depth 1, 2, 3)
- `count`/`for_each` warning behavior
- All 13 supported Terraform functions
- OpenTofu compatibility
- Error cases (missing files, malformed HCL, circular locals)

Each test uses a fixture directory with real `.tf` files — not string literals. This catches issues that synthetic test cases miss, like encoding edge cases or file-ordering dependencies.

## Lessons Learned

**1. Approximate parsing beats no parsing.** A parser that handles 95% of real-world Terraform is infinitely more useful than one that aims for 100% and ships never. Users are surprisingly tolerant of "this expression couldn't be resolved" as long as their main workflow works.

**2. Real-world Terraform is simpler than you'd expect.** We feared complex HCL would be everywhere. In practice, Lambda infrastructure is mostly straightforward: string attributes, simple variables, maybe a `jsonencode()` for IAM policies. The parser's subset covers the vast majority of projects.

**3. Error messages are a feature.** The first version silently skipped unparseable resources. Users thought their functions were missing. Adding clear "couldn't parse X because Y" messages eliminated an entire category of bug reports.

**4. Test with real projects.** Our fixture-based tests caught bugs that unit tests on string snippets never would. File ordering, encoding, relative paths, nested module resolution — these only surface with real directory structures.

**5. Don't fight the ecosystem.** We considered writing a proper HCL parser in Rust. We considered FFI to Go. We considered WASM. Every option was months of work for marginal improvement. The purpose-built approach shipped in days and handles real projects.

## Try It

```bash
# Install
brew install ConnerV42/tap/lambdaform
# or
cargo install lambdaform
# or
npx lambdaform

# Run
cd your-terraform-project
lambdaform start
```

It'll parse your `.tf` files, find your Lambda functions, and start a local server. If something doesn't parse, you'll get a clear error message telling you why.

The [source is on GitHub](https://github.com/ConnerV42/lambdaform) — the parser lives in `src/parser.rs` if you want to see the real implementation.

---

*[Conner Verret](https://github.com/ConnerV42) builds infrastructure tools in Rust. Lambdaform is open source and MIT licensed.*
