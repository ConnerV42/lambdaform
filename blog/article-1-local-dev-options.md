# Terraform + Lambda Local Dev in 2026: Your Options

*A neutral comparison of the tools available for developing AWS Lambda functions locally when your infrastructure is defined in Terraform.*

---

Every team using Terraform with AWS Lambda hits the same wall: how do you test locally? The ecosystem has evolved significantly, but each tool makes different trade-offs. Here's an honest look at where things stand in 2026.

## The Landscape

| | Reads .tf files | Docker required | Free | Startup time |
|---|---|---|---|---|
| **LocalStack** | No (deploys via provider) | Yes | Partially¹ | ~15-30s |
| **SAM CLI** | Beta | Yes | Yes | ~10-20s |
| **serverless-offline** | No | No | Yes | ~3-5s |
| **Lambdaform** | Yes | No² | Yes | <1s |

¹ LocalStack's free tier has been shrinking; account required as of March 2026.
² Docker optional, only needed for Java/JVM runtimes.

---

## LocalStack

**What it is:** A full AWS cloud emulator running in Docker. Supports dozens of AWS services beyond Lambda.

**How it works:** You run `terraform apply` against LocalStack's endpoints using a custom provider configuration. Your Lambda code runs inside Docker containers that mimic the AWS execution environment.

**Strengths:**
- Most complete AWS emulation — DynamoDB, SQS, S3, Step Functions, and more all work together
- Your deployment process is nearly identical to production
- Large community, extensive documentation
- Good for integration testing entire architectures

**Weaknesses:**
- **Docker dependency** — heavy, slow startup, resource-hungry
- **Account required** — the free tier has been shrinking steadily. Some services now require a Pro subscription
- **Doesn't read your Terraform** — you deploy *to* LocalStack, which means running `terraform apply` for every code change
- **Slow iteration loop** — change code → terraform apply → wait for deploy → test → repeat
- **Resource consumption** — running a mini-AWS on your laptop eats RAM and CPU

**Best for:** Teams that need to test complex multi-service architectures end-to-end, and are willing to pay for Pro.

---

## SAM CLI (with Terraform)

**What it is:** AWS's official serverless development toolkit. Added beta Terraform support in late 2023.

**How it works:** `sam local start-api --hook-name terraform` reads your Terraform state/plan and generates CloudFormation templates internally, then uses Docker to run your Lambda functions.

**Strengths:**
- Official AWS tooling — maintained by the Lambda team
- Supports Lambda layers, environment variables, event source mappings
- Works with step-through debugging (VS Code, IntelliJ)
- Free and open source

**Weaknesses:**
- **Terraform support is still beta** — two years in, and it's brittle. Breaks on module structures, conditional resources, complex variable interpolation
- **Docker required** — same startup overhead as LocalStack
- **CloudFormation translation layer** — your Terraform gets converted to CFN internally, which is a lossy translation. Edge cases abound
- **Slow feedback loop** — Docker container cold starts add seconds to every invocation
- **Confusing dual-config** — SAM wants a `template.yaml` alongside your `.tf` files, even in "Terraform mode"

**Best for:** Teams already using SAM for some projects, or those who need the official AWS debugging integration.

---

## serverless-offline

**What it is:** A Serverless Framework plugin that emulates API Gateway and Lambda locally.

**How it works:** Reads your `serverless.yml` and spins up a local HTTP server. Functions run as Node.js processes (or via Docker for other runtimes).

**Strengths:**
- Fast startup, no Docker needed (for Node.js/Python)
- Mature plugin ecosystem — DynamoDB, SQS, S3 plugins available
- Great developer experience if you're in the Serverless Framework ecosystem
- Active community, well-maintained

**Weaknesses:**
- **Doesn't support Terraform at all** — requires `serverless.yml`. If your infra is in Terraform, you're maintaining a parallel config
- **Framework lock-in** — the Serverless Framework has its own opinions about project structure, deployment, and packaging
- **Divergence risk** — your `serverless.yml` and `.tf` files can drift apart silently
- **Limited to API Gateway + Lambda** — no Step Functions, no WebSocket support (without additional plugins)

**Best for:** Teams fully committed to the Serverless Framework. Not practical for Terraform shops.

---

## Lambdaform

**What it is:** A Rust CLI that parses your `.tf` files directly and runs a local Lambda development server. No Docker, no translation layer.

**How it works:** Reads `aws_lambda_function`, `aws_api_gateway_*`, and `aws_apigatewayv2_*` resources from your Terraform files. Resolves variables from `.tfvars`, interpolates locals, follows module references. Starts a local HTTP server that routes to your actual handler code running as native processes.

**Strengths:**
- **Terraform-native** — reads your `.tf` files as the source of truth. No parallel config
- **No Docker** — functions run as native processes. Sub-second cold starts
- **Fast startup** — single binary, <1 second to parse and start serving
- **Hot reload** — watches `.tf` files, handler code, and config for changes
- **Broad runtime support** — Node.js, Python, Go, Rust, Java (Java uses Docker)
- **Extras** — request replay, cost estimation, infrastructure graph, TUI dashboard, VS Code extension

**Weaknesses:**
- **New project** — smaller community, less battle-tested than LocalStack or SAM
- **HCL parsing is approximate** — handles most Terraform patterns but doesn't implement the full HCL spec. Complex `for` expressions, `dynamic` blocks, and deeply nested conditionals may not parse
- **Lambda-focused** — doesn't emulate DynamoDB, SQS, S3, or other AWS services (you'd pair it with DynamoDB Local, ElasticMQ, etc.)
- **No production parity** — runs functions as native processes, not in Docker containers mimicking the Lambda execution environment

**Best for:** Terraform-first teams who want fast iteration on Lambda + API Gateway, and are comfortable using separate tools (DynamoDB Local, etc.) for other services.

---

## Which Should You Use?

**You need full AWS emulation** → LocalStack. Nothing else comes close for testing complex multi-service architectures.

**You're already on Serverless Framework** → serverless-offline. It's purpose-built for your workflow.

**You want official AWS tooling and use Terraform** → SAM CLI. But set expectations — the Terraform integration is beta and may frustrate you.

**You want fast iteration on Lambda + API Gateway with Terraform** → Lambdaform. It's the only tool that treats your `.tf` files as the source of truth without a translation layer.

**You want the best of both worlds** → Lambdaform for rapid development iteration + LocalStack (or real AWS) for integration testing. They solve different problems.

---

## The Bigger Picture

The fact that four different tools exist for this problem tells you something: local Lambda development with Terraform is genuinely hard. AWS hasn't solved it, and the community has been filling the gap with different approaches for years.

My advice: pick the tool that matches your iteration speed needs. If you're changing handler code 50 times a day, startup time and feedback loops matter more than production parity. If you're testing complex event-driven architectures, emulation fidelity matters more than speed.

The worst option is no local development at all — deploying to AWS for every change. Whatever tool gets you off that treadmill is the right one.

---

*[Conner Verret](https://github.com/ConnerV42) is the author of [Lambdaform](https://github.com/ConnerV42/lambdaform). This comparison aims to be fair — if you think something's inaccurate, [open a discussion](https://github.com/ConnerV42/lambdaform/discussions).*
