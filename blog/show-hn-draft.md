# Show HN Submission Draft

## Title (max 80 chars)
**Show HN: Lambdaform – Local Lambda dev server that reads your Terraform files**

Alternative titles:
- Show HN: Lambdaform – Run AWS Lambda locally by parsing your .tf files directly
- Show HN: Lambdaform – Terraform-native local Lambda emulator, no Docker needed

## URL
https://github.com/ConnerV42/lambdaform

## Text (optional, for Show HN — keep short, link to blog for details)

I use Terraform for Lambda infrastructure but hated the local dev experience. LocalStack needs Docker + an account, SAM CLI's Terraform support is still "beta" after 2+ years, and serverless-offline requires a separate config.

So I built Lambdaform: a Rust CLI that parses your .tf files directly (aws_lambda_function, aws_api_gateway_rest_api, etc.) and spins up a local HTTP server routing to your actual handler code.

Key points:
- Single binary, ~100ms cold start, ~3ms warm invocations
- Node.js, Python, Go, Rust run natively (no Docker). Java uses Docker.
- REST + HTTP API Gateways, Lambda authorizers, WebSocket APIs
- Hot reload on code and .tf changes
- Debugger integration, request replay, cost estimation
- OpenTofu compatible
- 125 tests, cross-platform CI

I've been dogfooding it with a real serverless app (3 Lambdas, API Gateway, DynamoDB, Bedrock) and it handles the full local dev loop.

MIT licensed, no telemetry, no paid tier.

Blog post with more context: [link to launch-post.md on blog/GitHub Pages]
Docs: https://connerv42.github.io/lambdaform/

---

## Posting Strategy

**When:** Tuesday or Wednesday, 9-10am ET (6-7am PT)
- HN engagement peaks mid-morning ET on weekdays
- Tue/Wed historically best for Show HN

**Prep before posting:**
- [ ] Ensure GitHub README is polished (current ✅)
- [ ] Ensure docs site is live and working
- [ ] Create 2-3 "good first issue" labels on GitHub
- [ ] Enable GitHub Discussions (General, Q&A, Ideas, Show and Tell)
- [ ] Have blog post published (GitHub Pages or dev.to)
- [ ] Test all install methods work (brew, cargo, npx)

**During launch day:**
- [ ] Post at target time
- [ ] Monitor and respond to comments promptly (first 2-3 hours critical)
- [ ] Be honest about limitations (Java needs Docker, not full AWS sim)
- [ ] Don't be defensive — engage genuinely

**After launch:**
- [ ] Cross-post to r/aws, r/terraform, r/serverless
- [ ] Tweet thread from @conner__v
- [ ] dev.to article (longer form)
