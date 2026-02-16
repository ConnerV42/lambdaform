# Architecture Overview

Lambdaform is a single Rust binary (~10.5k lines) organized into focused modules.

```
                    ┌─────────────┐
                    │   CLI/TUI   │  main.rs, tui.rs
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │   Server    │  server.rs
                    └──┬──────┬──┘
                       │      │
              ┌────────▼┐  ┌──▼────────┐
              │ Router  │  │ WebSocket │  router.rs, websocket.rs
              └────┬────┘  └─────┬─────┘
                   │             │
              ┌────▼─────────────▼────┐
              │     Runtime Engine    │  runtime.rs
              └────────┬─────────────┘
                       │
              ┌────────▼──────────┐
              │   Process Pool    │  pool.rs
              └───────────────────┘

  ┌──────────┐  ┌───────────┐  ┌──────────┐  ┌─────────┐
  │  Parser  │  │  Config   │  │ Watcher  │  │ History │
  │parser.rs │  │config.rs  │  │watcher.rs│  │history.rs│
  └──────────┘  └───────────┘  └──────────┘  └─────────┘
```

## Module Responsibilities

| Module | File | Lines | Purpose |
|--------|------|-------|---------|
| **CLI** | `main.rs` | 1,679 | Command parsing, orchestration |
| **Parser** | `parser.rs` | 2,780 | HCL parsing, resource extraction |
| **Runtime** | `runtime.rs` | 1,318 | Lambda invocation (Node.js, Python, Go) |
| **Server** | `server.rs` | 1,053 | HTTP server, request/response handling |
| **WebSocket** | `websocket.rs` | 596 | WebSocket API Gateway emulation |
| **Step Functions** | `stepfunctions.rs` | 526 | State machine visualization |
| **Config** | `config.rs` | 495 | Data structures, resolution |
| **TUI** | `tui.rs` | 428 | Terminal UI dashboard |
| **Pool** | `pool.rs` | 414 | Warm process management |
| **Trigger** | `trigger.rs` | 313 | SQS/SNS event simulation |
| **Project Config** | `project_config.rs` | 322 | `lambdaform.yaml` parsing |
| **Router** | `router.rs` | 249 | Path matching, route resolution |
| **History** | `history.rs` | 226 | Request recording/replay |
| **Watcher** | `watcher.rs` | 134 | File system monitoring |

## Request Flow

1. **HTTP request** arrives at the server (hyper)
2. **Router** matches the path and method to a route (v1 REST or v2 HTTP)
3. If an **authorizer** is configured, it runs first
4. **Runtime engine** prepares the Lambda event payload
5. **Process pool** provides a warm process (or spawns cold)
6. Handler code executes and returns a response
7. **Server** translates the Lambda response to HTTP
8. **History** records the request (if `--record` is enabled)
9. **TUI/Logger** displays the request info

## Key Design Decisions

- **No Docker dependency** — processes run natively for speed
- **Warm process pool** — pre-spawned processes eliminate cold start latency
- **Custom HCL parser** — avoids dependency on HashiCorp's Go tooling
- **Single binary** — no runtime dependencies beyond the language runtimes themselves
- **Async I/O** — Tokio runtime for concurrent request handling
