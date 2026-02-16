# CI/CD Integration

Lambdaform fits naturally into CI/CD pipelines, giving you automated integration testing of your Lambda functions against real Terraform/OpenTofu configurations — without deploying to AWS.

## Why Test with Lambdaform in CI?

- **Catch routing bugs before deploy** — verify API Gateway paths, methods, and authorizers resolve correctly
- **Test Lambda handler logic** — exercise your actual function code with realistic event payloads
- **Validate Terraform changes** — ensure `.tf` refactors don't break function wiring
- **Fast feedback** — no CloudFormation/Terraform apply, no AWS credentials needed
- **Replay production scenarios** — use recorded request histories as regression tests

## Installation in CI

### Binary download (fastest)

```bash
curl -fsSL https://github.com/ConnerV42/lambdaform/releases/latest/download/lambdaform-x86_64-unknown-linux-gnu \
  -o /usr/local/bin/lambdaform
chmod +x /usr/local/bin/lambdaform
```

### Via npm (cross-platform)

```bash
npm install -g lambdaform
```

### Via Homebrew (macOS runners)

```bash
brew install ConnerV42/tap/lambdaform
```

---

## GitHub Actions

### Basic: Validate + Integration Test

```yaml
name: Lambda Integration Tests
on:
  push:
    branches: [main]
  pull_request:

jobs:
  integration:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Install dependencies
        run: npm ci

      - name: Install Lambdaform
        run: |
          curl -fsSL https://github.com/ConnerV42/lambdaform/releases/latest/download/lambdaform-x86_64-unknown-linux-gnu \
            -o /usr/local/bin/lambdaform
          chmod +x /usr/local/bin/lambdaform

      - name: Validate Terraform config
        run: lambdaform validate --dir ./infra

      - name: Run integration tests
        run: |
          lambdaform start --dir ./infra --no-watch --json-log --port 3000 &
          LAMBDAFORM_PID=$!

          # Wait for server readiness
          for i in $(seq 1 30); do
            curl -sf http://localhost:3000/ > /dev/null 2>&1 && break
            sleep 1
          done

          # Run tests against local server
          npm test

          # Cleanup
          kill $LAMBDAFORM_PID 2>/dev/null || true
```

### Multi-Runtime Matrix

Test across Python, Node.js, and Go runtimes in parallel:

```yaml
name: Multi-Runtime Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        runtime: [nodejs20.x, python3.12, go1.x, provided.al2023]
        include:
          - runtime: nodejs20.x
            setup: |
              npm ci --prefix functions/node
          - runtime: python3.12
            setup: |
              pip install -r functions/python/requirements.txt
          - runtime: go1.x
            setup: |
              cd functions/go && go build -o bootstrap .
          - runtime: provided.al2023
            setup: |
              cd functions/rust && cargo build --release
              cp target/release/handler functions/rust/bootstrap

    steps:
      - uses: actions/checkout@v4

      - name: Setup runtimes
        run: ${{ matrix.setup }}

      - name: Install Lambdaform
        run: |
          curl -fsSL https://github.com/ConnerV42/lambdaform/releases/latest/download/lambdaform-x86_64-unknown-linux-gnu \
            -o /usr/local/bin/lambdaform
          chmod +x /usr/local/bin/lambdaform

      - name: Test ${{ matrix.runtime }}
        run: |
          lambdaform start --dir ./infra --no-watch --json-log &
          sleep 3
          ./scripts/test-${{ matrix.runtime }}.sh
```

### Replay-Based Regression Tests

Use recorded request histories as your test suite:

```yaml
name: Regression Tests
on: [push]

jobs:
  replay:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Lambdaform
        run: |
          curl -fsSL https://github.com/ConnerV42/lambdaform/releases/latest/download/lambdaform-x86_64-unknown-linux-gnu \
            -o /usr/local/bin/lambdaform
          chmod +x /usr/local/bin/lambdaform

      - name: Setup & start
        run: |
          npm ci
          lambdaform start --dir ./infra --no-watch --json-log &
          sleep 3

      - name: Replay recorded scenarios
        run: |
          # Replay from a checked-in request history file
          lambdaform replay --file tests/fixtures/requests.jsonl

          # Or replay with filters
          lambdaform replay --file tests/fixtures/requests.jsonl \
            --method POST --path "/api/users"
```

### Caching for Faster Builds

```yaml
      - name: Cache Lambdaform binary
        uses: actions/cache@v4
        id: lf-cache
        with:
          path: /usr/local/bin/lambdaform
          key: lambdaform-${{ runner.os }}-latest

      - name: Install Lambdaform
        if: steps.lf-cache.outputs.cache-hit != 'true'
        run: |
          curl -fsSL https://github.com/ConnerV42/lambdaform/releases/latest/download/lambdaform-x86_64-unknown-linux-gnu \
            -o /usr/local/bin/lambdaform
          chmod +x /usr/local/bin/lambdaform
```

### PR Comment with Test Results

```yaml
      - name: Run tests and capture output
        id: tests
        run: |
          lambdaform start --dir ./infra --no-watch --json-log > /tmp/lf.log 2>&1 &
          sleep 3
          npm test 2>&1 | tee /tmp/test-results.txt
          echo "result=$(tail -1 /tmp/test-results.txt)" >> "$GITHUB_OUTPUT"

      - name: Comment on PR
        if: github.event_name == 'pull_request'
        uses: actions/github-script@v7
        with:
          script: |
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: `## 🧪 Lambdaform Integration Tests\n\n\`\`\`\n${process.env.TEST_RESULT}\n\`\`\``
            })
```

---

## GitLab CI

### Basic Pipeline

```yaml
stages:
  - validate
  - test

variables:
  LAMBDAFORM_URL: "https://github.com/ConnerV42/lambdaform/releases/latest/download/lambdaform-x86_64-unknown-linux-gnu"

.install-lambdaform: &install-lambdaform
  before_script:
    - curl -fsSL "$LAMBDAFORM_URL" -o /usr/local/bin/lambdaform
    - chmod +x /usr/local/bin/lambdaform

validate:
  stage: validate
  image: alpine:latest
  <<: *install-lambdaform
  script:
    - lambdaform validate --dir ./infra

integration-test:
  stage: test
  image: node:20
  <<: *install-lambdaform
  script:
    - npm ci
    - lambdaform start --dir ./infra --no-watch --json-log &
    - |
      for i in $(seq 1 30); do
        curl -sf http://localhost:3000/ > /dev/null 2>&1 && break
        sleep 1
      done
    - npm test
  artifacts:
    when: on_failure
    paths:
      - /tmp/lambdaform*.log
    expire_in: 7 days
```

### Multi-Stage with Python + Node

```yaml
stages:
  - validate
  - test-node
  - test-python

validate:
  stage: validate
  image: alpine:latest
  before_script:
    - apk add --no-cache curl
    - curl -fsSL "$LAMBDAFORM_URL" -o /usr/local/bin/lambdaform
    - chmod +x /usr/local/bin/lambdaform
  script:
    - lambdaform validate --dir ./infra

test-node-functions:
  stage: test-node
  image: node:20
  before_script:
    - curl -fsSL "$LAMBDAFORM_URL" -o /usr/local/bin/lambdaform
    - chmod +x /usr/local/bin/lambdaform
    - npm ci
  script:
    - lambdaform start --dir ./infra --no-watch --json-log &
    - sleep 3
    - npm test -- --grep "node"

test-python-functions:
  stage: test-python
  image: python:3.12
  before_script:
    - curl -fsSL "$LAMBDAFORM_URL" -o /usr/local/bin/lambdaform
    - chmod +x /usr/local/bin/lambdaform
    - pip install -r requirements-test.txt
  script:
    - lambdaform start --dir ./infra --no-watch --json-log &
    - sleep 3
    - pytest tests/integration/

```

### Merge Request Integration

```yaml
integration-test:
  stage: test
  image: node:20
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
  before_script:
    - curl -fsSL "$LAMBDAFORM_URL" -o /usr/local/bin/lambdaform
    - chmod +x /usr/local/bin/lambdaform
  script:
    - npm ci
    - lambdaform validate --dir ./infra
    - lambdaform start --dir ./infra --no-watch --json-log &
    - sleep 3
    - npm test
  coverage: '/Statements\s*:\s*(\d+\.?\d*)%/'
```

---

## Writing Integration Tests

### Example: Node.js with Jest

```javascript
// tests/integration/api.test.js
const BASE_URL = process.env.LAMBDAFORM_URL || 'http://localhost:3000';

describe('API Integration', () => {
  test('GET /users returns 200', async () => {
    const res = await fetch(`${BASE_URL}/users`);
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('POST /users validates input', async () => {
    const res = await fetch(`${BASE_URL}/users`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({}), // missing required fields
    });
    expect(res.status).toBe(400);
  });

  test('GET /users/:id returns 404 for missing', async () => {
    const res = await fetch(`${BASE_URL}/users/nonexistent`);
    expect(res.status).toBe(404);
  });
});
```

### Example: Python with pytest

```python
# tests/integration/test_api.py
import os
import requests

BASE_URL = os.environ.get("LAMBDAFORM_URL", "http://localhost:3000")

def test_health_endpoint():
    r = requests.get(f"{BASE_URL}/health")
    assert r.status_code == 200

def test_create_item():
    r = requests.post(f"{BASE_URL}/items", json={"name": "test", "price": 9.99})
    assert r.status_code == 201
    data = r.json()
    assert data["name"] == "test"

def test_authorizer_rejects_missing_token():
    r = requests.get(f"{BASE_URL}/admin/dashboard")
    assert r.status_code == 401
```

### Example: Shell-Based Smoke Tests

```bash
#!/bin/bash
# scripts/smoke-test.sh
set -euo pipefail

BASE="${LAMBDAFORM_URL:-http://localhost:3000}"

echo "Testing health..."
curl -sf "$BASE/health" | grep -q "ok"

echo "Testing CORS preflight..."
STATUS=$(curl -s -o /dev/null -w '%{http_code}' \
  -X OPTIONS "$BASE/api/items" \
  -H "Origin: http://localhost:5173" \
  -H "Access-Control-Request-Method: POST")
[ "$STATUS" = "200" ] || [ "$STATUS" = "204" ]

echo "Testing POST /api/items..."
curl -sf -X POST "$BASE/api/items" \
  -H "Content-Type: application/json" \
  -d '{"name":"widget"}' | grep -q "widget"

echo "All smoke tests passed ✅"
```

---

## Best Practices

### Server Readiness

Don't rely on `sleep`. Use a readiness loop:

```bash
# Wait up to 30 seconds for Lambdaform to be ready
for i in $(seq 1 30); do
  curl -sf http://localhost:3000/ > /dev/null 2>&1 && break
  [ "$i" -eq 30 ] && echo "Lambdaform failed to start" && exit 1
  sleep 1
done
```

### Flags for CI

| Flag | Purpose |
|------|---------|
| `--no-watch` | Disable file watching (no inotify overhead) |
| `--json-log` | Machine-parseable structured logs |
| `--port PORT` | Explicit port (avoid conflicts) |

### Use `lambdaform validate` as a Fast Gate

Run `validate` as a separate, early step. It parses your Terraform and checks for configuration errors in seconds — no server startup needed. Fail fast before heavier integration tests.

### Artifact Collection

Always capture Lambdaform logs on failure. In GitHub Actions:

```yaml
      - name: Upload logs on failure
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: lambdaform-logs
          path: /tmp/lf.log
```

### Pin Versions for Reproducibility

For production pipelines, pin to a specific release instead of `latest`:

```bash
LAMBDAFORM_VERSION="0.6.0"
curl -fsSL "https://github.com/ConnerV42/lambdaform/releases/download/v${LAMBDAFORM_VERSION}/lambdaform-x86_64-unknown-linux-gnu" \
  -o /usr/local/bin/lambdaform
```

### Parallel Test Suites

Run Lambdaform on different ports for parallel test jobs:

```yaml
strategy:
  matrix:
    suite: [auth, crud, websocket]
    include:
      - suite: auth
        port: 3001
      - suite: crud
        port: 3002
      - suite: websocket
        port: 3003
```

---

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| Server won't start in CI | Check that runtimes (node, python) are installed in the CI image |
| `validate` passes but `start` fails | Lambda handler files may be missing — check paths in `.tf` |
| Tests flaky / connection refused | Use readiness loop instead of `sleep` |
| Permission denied on binary | Ensure `chmod +x` after download |
| Wrong architecture | Use `x86_64` for standard runners, `aarch64` for ARM64 runners |
