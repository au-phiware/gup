# Performance

Gup's performance budgets, how they are measured and on which machine are in
[`crates/gup-core/PERF_BUDGETS.md`](../crates/gup-core/PERF_BUDGETS.md). It is
the only current performance reference; the guide that was here described the
render path deleted in GUP-488.

- `mask perf-budget` - the budgets' pass/fail table (needs a display and a GPU)
- `mask gup-core-window` - window parity and the 100K-point zoom benchmark
- `mask wasm-size` and `mask wasm-attrib` - WASM size against its budget, and
  where the bytes go

Level of detail and density aggregation are parked in `crates/gup-culling-lod`
until track N5 (strategic review 2026-11).
