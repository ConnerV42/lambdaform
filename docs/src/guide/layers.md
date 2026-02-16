# Lambda Layers

Lambdaform resolves Lambda layer paths so your functions can import layer dependencies locally.

## How It Works

When your Terraform defines layers:

```hcl
resource "aws_lambda_layer_version" "utils" {
  layer_name          = "utils"
  compatible_runtimes = ["nodejs20.x"]
  filename            = "layers/utils.zip"
  source_code_hash    = filebase64sha256("layers/utils.zip")
}

resource "aws_lambda_function" "api" {
  layers  = [aws_lambda_layer_version.utils.arn]
  runtime = "nodejs20.x"
  handler = "index.handler"
}
```

Lambdaform adds the layer's source directory to the runtime path:

- **Node.js:** Added to `NODE_PATH` (looks for `nodejs/node_modules/` in the layer)
- **Python:** Added to `PYTHONPATH` (looks for `python/` in the layer)

## Layer Directory Structure

Layers follow the [AWS Lambda layer path conventions](https://docs.aws.amazon.com/lambda/latest/dg/chapter-layers.html):

```
layers/utils/
├── nodejs/
│   └── node_modules/
│       └── shared-utils/
│           └── index.js
```

> **Tip:** For local development, keep layer contents extracted (not zipped). Lambdaform reads the directory — it doesn't extract zip files.
