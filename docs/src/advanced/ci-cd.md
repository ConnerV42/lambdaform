# CI/CD Integration

Use Lambdaform in your CI pipeline for automated integration testing.

## GitHub Actions

```yaml
name: Integration Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Install Lambdaform
        run: |
          curl -L https://github.com/ConnerV42/lambdaform/releases/latest/download/lambdaform-x86_64-unknown-linux-gnu -o lambdaform
          chmod +x lambdaform
          sudo mv lambdaform /usr/local/bin/

      - name: Validate Terraform
        run: lambdaform validate --dir ./infra

      - name: Start server and test
        run: |
          lambdaform start --dir ./infra --no-watch --json-log &
          sleep 2
          curl -f http://localhost:3000/health
          # Run your integration test suite
          npm test
```

## GitLab CI

```yaml
integration-test:
  image: node:20
  script:
    - curl -L https://github.com/ConnerV42/lambdaform/releases/latest/download/lambdaform-x86_64-unknown-linux-gnu -o /usr/local/bin/lambdaform
    - chmod +x /usr/local/bin/lambdaform
    - lambdaform validate
    - lambdaform start --no-watch --json-log &
    - sleep 2
    - npm test
```

## Tips

- Use `--no-watch` to disable file watching in CI
- Use `--json-log` for machine-parseable output
- Run `lambdaform validate` as a fast pre-check before starting the server
- Use `lambdaform replay` to replay recorded test scenarios
