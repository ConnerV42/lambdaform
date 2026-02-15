#!/bin/bash
# Fast scripted demo for asciinema recording
set -e

LAMBDAFORM="/mnt/ssd/clawdbot/lambdaform/target/debug/lambdaform"
FIXTURE="/mnt/ssd/clawdbot/lambdaform/tests/fixtures/simple-node"

typeit() {
  local text="$1"
  for ((i=0; i<${#text}; i++)); do
    printf '%s' "${text:$i:1}"
    sleep 0.03
  done
  echo
}

prompt() { printf '\033[1;32m❯\033[0m '; }

clear
echo ""
echo "  🚀 Lambdaform — Local Lambda dev with your Terraform"
echo ""
sleep 1

prompt; typeit "cat main.tf | head -15"
sleep 0.2
head -15 "$FIXTURE/main.tf"
sleep 1

prompt; typeit "lambdaform validate"
sleep 0.2
$LAMBDAFORM validate -d "$FIXTURE" 2>&1 | grep -v "^\\[2m"
sleep 1

prompt; typeit "lambdaform start &"
sleep 0.2
$LAMBDAFORM start -d "$FIXTURE" 2>&1 | grep -v "^\\[2m" &
SERVER_PID=$!
sleep 2

prompt; typeit 'curl -s localhost:3000/hello?name=World | jq .'
sleep 0.2
curl -s "localhost:3000/hello?name=World" | jq .
sleep 1

prompt; typeit 'curl -s -X POST localhost:3000/echo -d "{\"msg\":\"hi\"}" | jq .'
sleep 0.2
curl -s -X POST "localhost:3000/echo" -d '{"msg":"hi"}' | jq .
sleep 1

kill $SERVER_PID 2>/dev/null; wait $SERVER_PID 2>/dev/null || true

echo ""
echo "  ✨ No Docker. No CloudFormation. Just Terraform + code."
echo "  📦 cargo install lambdaform | brew install lambdaform"
echo ""
sleep 2
