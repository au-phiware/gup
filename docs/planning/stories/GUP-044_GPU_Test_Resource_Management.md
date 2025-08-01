# GUP-044: GPU Test Resource Management

## Story

**As a** developer running tests  
**I want** GPU tests to run reliably in parallel without resource conflicts  
**So that** the test suite is fast and doesn't require special execution flags

## Background

During GUP-027 implementation, we discovered that GPU tests suffer from resource
contention when run in parallel, causing segmentation faults. The current
workaround is running tests with `--test-threads=1`, which works but slows down
the development cycle.

The root cause is multiple GPU contexts competing for hardware resources, likely
involving:

- Multiple device/adapter requests simultaneously
- Buffer creation conflicts
- GPU memory allocation races
- WebGPU instance management issues

## Acceptance Criteria

### Resource Management Strategy

- [ ] GPU tests run reliably in parallel without crashes
- [ ] Remove need for `--test-threads=1` workaround
- [ ] Maintain test isolation (tests don't affect each other)
- [ ] Preserve existing test functionality and coverage

### Technical Implementation

- [ ] Implement shared GPU context pool for tests
- [ ] Add proper resource cleanup in test teardown
- [ ] Handle resource conflicts gracefully with retries
- [ ] Provide test utilities for safe context creation

### Performance Requirements

- [ ] Test suite runs in <30 seconds (current baseline)
- [ ] Parallel execution shows measurable speedup vs sequential
- [ ] No test flakiness or intermittent failures
- [ ] Memory usage remains stable across test runs

### Developer Experience

- [ ] Standard `cargo test` command works without flags
- [ ] Clear error messages for resource conflicts
- [ ] Test utilities are easy to use in new tests
- [ ] Documentation explains GPU testing best practices

## Implementation Notes

### Root Cause Analysis

The segmentation fault occurs because:

1. Multiple tests create WebGPU instances simultaneously
2. GPU drivers may not handle concurrent adapter requests
3. Buffer allocation conflicts between test contexts
4. Improper cleanup of GPU resources

### Proposed Solutions

#### Option 1: Shared Context Pool

```rust
lazy_static! {
    static ref TEST_CONTEXT_POOL: Mutex<Vec<Arc<GupContext>>> = Mutex::new(Vec::new());
}

async fn get_test_context() -> Arc<GupContext> {
    // Lease context from pool or create new one with backoff
}
```

#### Option 2: Test Context Manager

```rust
struct TestContextManager {
    contexts: HashMap<ThreadId, Arc<GupContext>>,
    creation_lock: Mutex<()>,
}

impl TestContextManager {
    async fn get_context_for_thread() -> Arc<GupContext>;
}
```

#### Option 3: Resource Semaphore

```rust
static GPU_RESOURCE_SEMAPHORE: Semaphore = Semaphore::new(4); // Limit concurrent contexts

#[tokio::test]
async fn gpu_test() {
    let _permit = GPU_RESOURCE_SEMAPHORE.acquire().await;
    // Test runs with limited concurrency
}
```

### Integration Strategy

1. **Phase 1**: Implement context pooling for new tests
2. **Phase 2**: Migrate existing tests to use pool
3. **Phase 3**: Remove `--test-threads=1` requirement
4. **Phase 4**: Optimize pool size and resource management

## Dependencies

- **Depends on**: GUP-027 (GPU Blend State Integration) - Complete
- **Relates to**: All GPU-related stories that add tests

## Definition of Done

- [ ] All GPU tests pass reliably with `cargo test` (no special flags)
- [ ] Test suite performance is maintained or improved
- [ ] No segmentation faults or resource conflicts
- [ ] Test utilities are documented and easy to use
- [ ] CI/CD pipeline updated to use standard test command

## Estimated Effort

**3-4 days** - Medium-high complexity due to GPU resource management challenges

## Success Metrics

- Zero test failures due to resource conflicts
- Test suite speedup from parallel execution
- Improved developer experience (no special flags needed)
- Stable memory usage across test runs

## Notes

This issue was discovered during GUP-027 and affects developer productivity.
While the `--test-threads=1` workaround functions, it slows development and may
hide concurrency issues in the actual GPU code.

The solution should be robust enough to handle various GPU hardware and drivers
while maintaining test isolation and reliability.
