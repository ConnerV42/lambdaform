# Process Pool

The warm process pool (`pool.rs`) keeps pre-spawned runtime processes ready for instant invocation.

## How It Works

1. On first invocation of a function, a process is spawned (cold start ~100ms)
2. After the invocation completes, the process is returned to the pool
3. Subsequent invocations reuse the warm process (~3ms)
4. Pool size is managed per-function

## Pool Lifecycle

- **Startup:** No processes are pre-spawned (lazy initialization)
- **Hot reload:** All pool processes are killed and the pool is flushed
- **Shutdown:** SIGINT triggers graceful shutdown — all pool workers are killed
- **Debug mode:** Pool is disabled; every invocation gets a fresh process

## Why Not Docker?

Docker adds ~500ms-2s overhead per cold start. Native process spawning with a warm pool achieves:
- ~100ms cold starts (first invocation)
- ~3ms warm invocations (subsequent)
- Zero memory overhead from container runtime
- No Docker daemon dependency
