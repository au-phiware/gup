# GUP-350: Live Profiling WebSocket Server

**Status**: 💡 New

## Story Overview

**Title**: Live Profiling WebSocket Server **Epic**: Phase 1 Initiative 1 - Core
GPU Primitives and Selection API **Priority**: Low **Story Points**: 5

## Context

GUP-148 implemented profiling data export and a static HTML dashboard generator.
While the static dashboard captures a snapshot of profiler state, some workflows
benefit from a continuously-updating browser dashboard that refreshes in real
time as frames are rendered.

## User Story

**As a** Gup application developer **I want** a live profiling dashboard that
updates in real time **So that** I can observe performance characteristics as I
interact with my visualization

## Acceptance Criteria

### AC1: Embedded HTTP Server

- [ ] Feature-gated `profiling-server` to avoid adding networking dependencies
      to the core library
- [ ] Lightweight HTTP server (e.g., `tiny_http` or `hyper`) serving the
      dashboard HTML
- [ ] Configurable bind address and port

### AC2: WebSocket Data Push

- [ ] WebSocket endpoint that pushes frame stats as JSON on each `end_frame()`
- [ ] Client-side JavaScript in the dashboard HTML that receives WebSocket
      messages and updates charts
- [ ] Auto-reconnect on connection loss

### AC3: Integration with PerformanceProfiler

- [ ] Simple API: `profiler.serve_dashboard("127.0.0.1:9090")`
- [ ] Non-blocking — server runs in a background thread
- [ ] Graceful shutdown when the profiler is dropped or disabled

## Dependencies

- GUP-148: Profiling Data Export and Visualization (completed)

## Technical Requirements

```rust
#[cfg(feature = "profiling-server")]
pub struct ProfilingServer {
    addr: SocketAddr,
    handle: JoinHandle<()>,
}

#[cfg(feature = "profiling-server")]
impl PerformanceProfiler {
    pub fn serve_dashboard(&mut self, addr: &str) -> GupResult<ProfilingServer>;
}
```

## Testing Strategy

- Unit tests for WebSocket message serialization
- Integration test launching server, connecting via WebSocket, verifying data
  push
- Verify feature gate doesn't affect non-server builds

## Success Metrics

- [ ] Dashboard updates within 50ms of frame completion
- [ ] <1% overhead when server has no connected clients
- [ ] Works across Chrome, Firefox, Safari

## Risk Assessment

- **Dependency bloat**: Feature-gating mitigates this — networking crates only
  included when `profiling-server` is enabled.
- **Thread safety**: The profiler must safely share state with the server
  thread. Consider using `Arc<Mutex<>>` or a lock-free channel for frame data.

## Definition of Done

- [ ] All acceptance criteria met
- [ ] Feature-gated behind `profiling-server`
- [ ] Comprehensive test suite
- [ ] Documentation with usage examples
- [ ] Zero impact on default (non-server) builds
