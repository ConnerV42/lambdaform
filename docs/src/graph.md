# Infrastructure Graph

Visualize relationships between your Lambda functions, API Gateways, DynamoDB tables, SQS queues, SNS topics, layers, and Step Functions.

## Usage

```bash
# ASCII art (default) — colorful terminal output
lambdaform graph

# Graphviz DOT format — pipe to dot for PNG/SVG
lambdaform graph --format dot > infra.dot
dot -Tpng infra.dot -o infra.png

# JSON — for programmatic consumption
lambdaform graph --format json

# Specify directory and var files
lambdaform graph -d ./infra --var-file prod.tfvars
```

## Output Formats

### ASCII (default)

Colored terminal output grouped by resource type, showing each resource with its properties and connections (incoming/outgoing edges).

### DOT (Graphviz)

Standard [Graphviz DOT](https://graphviz.org/) format. Resources are clustered by type with AWS-inspired colors. Render with:

```bash
lambdaform graph --format dot | dot -Tsvg -o infra.svg
```

### JSON

Machine-readable output with `nodes`, `edges`, and `summary` fields. Useful for CI pipelines, custom visualizations, or integration with other tools.

## What's Visualized

| Resource | Relationships Detected |
|----------|----------------------|
| **Lambda** | API Gateway routes, event source mappings, layer usage, DynamoDB env var references |
| **API Gateway** | Routes to Lambda functions, authorizer functions |
| **DynamoDB** | Stream triggers to Lambda, env var references from Lambda |
| **SQS** | Event source mappings to Lambda |
| **SNS** | Event source mappings to Lambda |
| **Layers** | Usage by Lambda functions |
| **Step Functions** | Lambda invocations detected in ASL definitions |

## Tips

- **DynamoDB references** are detected heuristically from Lambda environment variables. If a var's value matches a DynamoDB table's resource name or table name, an edge is drawn.
- **Step Functions → Lambda** edges are detected by scanning the ASL definition for Lambda resource name references.
- Combine with `lambdaform config --json` for raw parsed data.
