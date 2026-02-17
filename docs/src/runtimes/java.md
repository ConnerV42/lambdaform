# Java Runtime Support

Lambdaform supports Java Lambda functions via Docker-based execution using AWS Lambda base images.

## Requirements

- **Docker** must be installed and running
- Internet access (first run only, to pull base images)

## Supported Versions

| Runtime | Docker Image |
|---------|-------------|
| `java8.al2` | `public.ecr.aws/lambda/java:8.al2` |
| `java11` | `public.ecr.aws/lambda/java:11` |
| `java17` | `public.ecr.aws/lambda/java:17` |
| `java21` | `public.ecr.aws/lambda/java:21` |

## How It Works

1. Lambdaform pulls the appropriate AWS Lambda Java base image (cached after first use)
2. Creates a container with your compiled classes/JAR mounted at `/var/task`
3. Sets the handler via the `_HANDLER` environment variable
4. Starts the container and POSTs your event to the built-in Runtime Interface Client
5. Returns the response and cleans up the container

## Example Terraform Config

```hcl
resource "aws_lambda_function" "my_java_fn" {
  function_name = "my-java-function"
  runtime       = "java21"
  handler       = "com.example.Handler::handleRequest"
  filename      = "target/my-function.jar"
  timeout       = 30
  memory_size   = 512

  environment {
    variables = {
      TABLE_NAME = aws_dynamodb_table.my_table.name
    }
  }
}
```

## Source Directory

Your `source_path` should point to a directory containing compiled Java classes or a JAR file. For Maven/Gradle projects, this is typically:

- **Maven:** `target/classes/` or the shaded JAR
- **Gradle:** `build/classes/java/main/` or the shadow JAR

## Troubleshooting

### "Failed to connect to Docker daemon"
Ensure Docker is running: `docker info`

### Image pull fails
Check internet connectivity. The images are pulled from `public.ecr.aws/lambda/java`.

### Handler not found
Ensure your compiled `.class` files are in the mounted directory and the handler format is correct: `com.example.Handler::methodName`

### Timeout issues
Java Lambda cold starts can be slow. Increase `timeout` in your Terraform config (30-60s recommended for Java).
