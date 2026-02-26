# GUP-226: WebAssembly Axis Performance Validation

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Automatic Scale and
Axis System **Priority**: Low **Story Points**: 3 **Status**: 📋 Planned

## Overview

Run the cross-platform axis performance benchmarks (from GUP-206) in an actual
WebAssembly/browser environment using `wasm-pack test --headless --chrome` to
validate the 2 ms performance budget and the more aggressive LOD thresholds
(250/130/65 px) with real browser WebGPU overhead.

## Context

GUP-206 established the cross-platform axis performance validation
infrastructure and tuned WebAssembly-specific thresholds based on expected
overhead. However, these thresholds were set analytically — they have not been
validated against actual browser WebGPU execution. Real-world browser
performance may differ due to JIT warm-up, JS/Wasm bridge latency, and
browser-specific WebGPU driver implementations.

## User Story

> "As a developer building a web-deployed chart, I want to know that axis
> performance meets the 2 ms budget in real browser WebGPU so that my charts are
> smooth without manual tuning."

## Acceptance Criteria

- [ ] `wasm-pack test` runs the axis performance benchmarks in a headless
      browser
- [ ] Actual WebAssembly median times collected and documented
- [ ] 2 ms performance budget validated with real browser data
- [ ] LOD thresholds adjusted if actual variance exceeds 2× from Linux baseline
- [ ] CI workflow includes WebAssembly benchmark job (headless Chrome)

## Technical Tasks

1. Create a wasm-compatible benchmark harness (Criterion does not support wasm)
2. Port the 8 axis benchmarks to a wasm-compatible test
3. Set up headless Chrome in CI for `wasm-pack test`
4. Collect real measurements and compare to Linux baseline
5. Tune WebAssembly LOD thresholds if needed

## Dependencies

- **GUP-206**: Cross-Platform Axis Performance Validation ✅
- **Infrastructure**: Headless Chrome in CI

## Testing Strategy

- `wasm-pack test --headless --chrome` for WebAssembly benchmarks
- Compare results against Linux baseline from GUP-206
- Validate 2× maximum variance threshold

## Success Metrics

- All 8 benchmarks complete successfully in browser environment
- Median times documented and within 2 ms budget
- Cross-platform variance within 2× of Linux baseline

## Risk Assessment

- **Browser WebGPU availability**: Not all CI environments support WebGPU in
  headless mode. May need to use `--features webgl` fallback.
- **Benchmark stability**: JIT warm-up in browsers may cause high variance in
  first few iterations.
- **wasm-pack compatibility**: Some Criterion features may not work in wasm.

## Definition of Done

- [ ] WebAssembly benchmarks run in headless browser
- [ ] Results documented alongside Linux baseline
- [ ] Thresholds adjusted if variance exceeds limits
- [ ] CI workflow includes WebAssembly job
