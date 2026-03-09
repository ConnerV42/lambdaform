# Cost Estimation

Lambdaform can estimate your AWS Lambda costs based on local request history, giving you a clear picture of what your workload would cost in production — before you deploy.

## How It Works

1. **Record requests** — Run `lambdaform start --record` to capture invocations to `.lambdaform/history.jsonl`
2. **Estimate costs** — Run `lambdaform cost` to analyze the history and calculate costs

Lambdaform uses official AWS Lambda pricing:

| Component | x86 Price | ARM/Graviton Price |
|-----------|-----------|-------------------|
| Requests | $0.20 per 1M | $0.20 per 1M |
| Duration | $0.0000166667/GB-s | $0.0000133334/GB-s |

Free tier (per month): 1M requests + 400,000 GB-seconds.

## Quick Start

```bash
# Start recording requests
lambdaform start --record

# Make some requests (curl, browser, tests, etc.)
curl http://localhost:3000/api/users
curl -X POST http://localhost:3000/api/users -d '{"name":"test"}'

# Stop the server, then estimate costs
lambdaform cost
```

## Output

The cost command produces a per-function breakdown:

```
╭─────────────────────────────────────────────╮
│           Lambda Cost Estimate              │
├──────────────┬──────┬───────┬───────┬───────┤
│ Function     │ Invs │ Avg   │ P95   │ Cost  │
├──────────────┼──────┼───────┼───────┼───────┤
│ get_users    │   42 │ 12ms  │ 28ms  │ $0.00 │
│ create_user  │   18 │ 45ms  │ 89ms  │ $0.00 │
│ process_order│    5 │ 230ms │ 410ms │ $0.00 │
├──────────────┴──────┴───────┴───────┴───────┤
│ Total: 65 invocations                       │
│ Monthly projection: ~195,000 invs           │
│ Estimated monthly cost: $0.42               │
│ After free tier: $0.00                      │
╰─────────────────────────────────────────────╯
```

Each function shows:
- **Invocations** — How many times it was called
- **Avg/P95/Max duration** — Performance characteristics
- **GB-seconds** — Memory × duration (the billing unit)
- **Cost** — Estimated AWS cost for those invocations

## ARM/Graviton Pricing

If your Lambda functions use Graviton (ARM) architecture, pass `--arch arm` for accurate pricing:

```bash
lambdaform cost --arch arm
```

ARM pricing is ~20% cheaper than x86.

## Monthly Projections

Lambdaform extrapolates from your recorded history to project monthly costs. The projection includes:

- **Projected invocations** — Scaled to 30 days based on your observation window
- **Projected GB-seconds** — Memory-weighted duration projection
- **Free tier savings** — How much the AWS free tier would offset
- **Net monthly cost** — What you'd actually pay

## JSON Output

For programmatic use or CI/CD integration:

```bash
lambdaform cost --json
```

Returns structured JSON with all cost data, suitable for dashboards or budget alerts.

## Tips

- **Record realistic workloads** — The more representative your test traffic, the better the estimate
- **Check memory settings** — Cost scales linearly with configured memory (default 128MB). Check your Terraform `memory_size` settings
- **Use with CI** — Record during integration tests, then run `lambdaform cost --json` to track cost trends over time
- **Free tier is generous** — Most small projects fall entirely within the free tier (1M requests + 400K GB-seconds/month)
