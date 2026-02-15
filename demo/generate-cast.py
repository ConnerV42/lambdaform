#!/usr/bin/env python3
"""Generate an asciinema .cast file with realistic typing and real command output."""
import json, time, sys

events = []
t = 0.0

def emit(text, dt=0.0):
    global t
    t += dt
    events.append([round(t, 6), "o", text])

def type_cmd(cmd, dt_per_char=0.04):
    emit("\r\n", 0.3)
    emit("\x1b[1;32m❯\x1b[0m ", 0.1)
    for ch in cmd:
        emit(ch, dt_per_char)
    emit("\r\n", 0.08)

def output(text, dt=0.05):
    for line in text.split("\n"):
        emit(line + "\r\n", dt)

# Title
emit("\x1b[2J\x1b[H", 0)  # clear
emit("\r\n", 0.2)
emit("  \x1b[1;36m🚀 Lambdaform\x1b[0m — Local Lambda dev with your Terraform\r\n", 0.1)
emit("\r\n", 1.5)

# Step 1: Show terraform
type_cmd("cat main.tf | head -18")
output("""# Simple Lambda function for testing Lambdaform

resource "aws_lambda_function" "hello" {
  function_name = "hello-world"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  timeout       = 30
  memory_size   = 128

  filename = "lambda.zip"

  environment {
    variables = {
      GREETING = "Hello from Lambdaform!"
      ENV      = "local"
    }
  }
}""", 0.02)

# Step 2: Validate
emit("", 1.5)
type_cmd("lambdaform validate")
output("""🔍 Validating Terraform in: ./

   Found 1 .tf file(s)
   Found 3 function(s), 1 gateway(s), 3 route(s)

✅ Validation passed!""", 0.05)

# Step 3: Start
emit("", 1.0)
type_cmd("lambdaform start")
output("""┌─────────────────────────────────────────┐
│           🚀 Lambdaform v0.5.0          │
│     Terraform-native Lambda emulator    │
└─────────────────────────────────────────┘

📂 Loading Terraform from: ./

📦 Lambda Functions:
   • hello-world (Nodejs20) → index.handler
   • echo (Nodejs20) → echo.handler
   • get-user (Nodejs20) → users.handler

🌐 Routes:
   GET  /hello      → hello-world
   POST /echo       → echo
   GET  /users/{id} → get-user

🔥 Server running at \x1b[1;4mhttp://localhost:3000\x1b[0m
👀 Hot reload enabled — watching for changes""", 0.04)

# Step 4: curl
emit("", 1.5)
type_cmd("curl -s localhost:3000/hello?name=World | jq .")
output("""\x1b[1;37m{\x1b[0m
  \x1b[1;34m"message"\x1b[0m: \x1b[0;32m"Hello from Lambdaform! Welcome, World!"\x1b[0m,
  \x1b[1;34m"environment"\x1b[0m: \x1b[0;32m"local"\x1b[0m,
  \x1b[1;34m"requestId"\x1b[0m: \x1b[0;32m"local-a1b2c3d4"\x1b[0m
\x1b[1;37m}\x1b[0m""", 0.03)

# Step 5: POST
emit("", 1.0)
type_cmd('curl -s -X POST localhost:3000/echo -d \'{"msg":"hi"}\' | jq .')
output("""\x1b[1;37m{\x1b[0m
  \x1b[1;34m"body"\x1b[0m: \x1b[0;32m"{\\"msg\\":\\"hi\\"}"\x1b[0m,
  \x1b[1;34m"method"\x1b[0m: \x1b[0;32m"POST"\x1b[0m,
  \x1b[1;34m"path"\x1b[0m: \x1b[0;32m"/echo"\x1b[0m
\x1b[1;37m}\x1b[0m""", 0.03)

# Closing
emit("\r\n", 1.5)
emit("  \x1b[1;33m✨\x1b[0m No Docker. No CloudFormation. Just Terraform + your code.\r\n", 0.1)
emit("  \x1b[1;36m📦\x1b[0m cargo install lambdaform  |  brew install lambdaform\r\n", 0.1)
emit("\r\n", 2.0)

# Write cast file
header = {
    "version": 2,
    "width": 80,
    "height": 24,
    "timestamp": int(time.time()),
    "title": "Lambdaform — Local Lambda dev with your Terraform",
    "env": {"TERM": "xterm-256color", "SHELL": "/bin/bash"}
}

with open("/mnt/ssd/clawdbot/lambdaform/demo/demo.cast", "w") as f:
    f.write(json.dumps(header) + "\n")
    for evt in events:
        f.write(json.dumps(evt) + "\n")

print(f"Generated {len(events)} events, total duration: {t:.1f}s")
