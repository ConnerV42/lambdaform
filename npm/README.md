# lambdaform

Terraform-native local Lambda emulator — no Docker, no CloudFormation.

## Quick Start

```bash
npx lambdaform serve
```

Or install globally:

```bash
npm install -g lambdaform
lambdaform serve
```

## What is this?

Lambdaform reads your Terraform files directly and runs your Lambda functions locally. No Docker, no SAM templates, no CloudFormation conversion.

**Supports:**
- Node.js, Python, Go runtimes
- API Gateway v1 (REST) and v2 (HTTP)
- WebSocket API Gateway
- Lambda Layers
- Lambda Authorizers
- Hot reload
- Debugger integration (Node.js + Python)

## More Info

See the full documentation at [github.com/ConnerV42/lambdaform](https://github.com/ConnerV42/lambdaform).
