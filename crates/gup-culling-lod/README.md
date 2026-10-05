# gup-culling-lod (experimental, unwired)

GPU culling and level-of-detail (LOD) code, quarantined out of the `gup` crate
by GUP-390.

## Why this crate exists

The
[October 2026 strategic review](../../docs/planning/STRATEGIC_REVIEW_2026-10.md)
found that nothing in the `gup` render path or chart builders calls this code.
Unlike the other unwired subsystems, which GUP-389 deleted, it is tested GPU
engineering that later work is expected to reuse (Decision 1). It was moved here
rather than deleted, so that:

- it no longer adds to the `gup` crate's compile time or public API;
- it stays buildable and tested, so it does not rot silently;
- [RFC-001](../../docs/planning/rfcs/RFC-001_Core_Architecture.md) can draw from
  it for GPU column chunk culling and LOD in `crates/gup-core` (strategic review
  tracks T3 and T7, "Wire GPU culling and LOD into core").

Treat it as a parts bin, not a dependency. Do not wire new `gup` features onto
it. Port the pieces you need into `gup-core` and delete them here.

## What is in it

Module paths match the ones these modules had inside `gup`:

| Module                           | Contents                                                                                          |
| -------------------------------- | ------------------------------------------------------------------------------------------------- |
| `mark::batch_renderer`           | `InstancedBatchRenderer`, CPU `CullingManager`, `InstanceAttributes`, `Viewport2D`, `LodLevel`    |
| `mark::compute_instance_filter`  | GPU frustum culling and LOD classification, prefix-sum compaction, indirect draw arguments        |
| `mark::occlusion_culler`         | Hi-Z occlusion culling with a coverage budget                                                     |
| `mark::radix_sort`               | GPU 8-bit radix sort for depth ordering                                                           |
| `mark::unified_culling_pipeline` | Frustum and occlusion culling in one compute submission                                           |
| `lod` (`selection`, `streaming`) | GPU-aggregated LOD pyramid, tier selection and streaming LOD with a memory budget                 |
| `renderer`                       | Adaptive viewport renderer: per-frame tier selection, cross-fade, viewport culling, debug overlay |

WGSL shaders are in `src/shaders/`. The crate depends on `gup` for `GupContext`,
`GupError`, the buffer pool and the `Mark` trait.

## Building and testing

It is a workspace member but not a default member, so a plain `cargo build` or
`cargo test` at the repository root skips it. Build it explicitly:

```bash
cargo check -p gup-culling-lod --all-targets --all-features
cargo test -p gup-culling-lod -- --test-threads=1
cargo run -p gup-culling-lod --example lod_pyramid_debug
cargo bench -p gup-culling-lod --bench compute_filter_benchmarks
cargo bench -p gup-culling-lod --features gpu-bench --bench lod_pyramid
```

CI runs the check and the tests in the visual-regression workflow, and the
examples smoke test (`mask smoke-examples`) runs the three examples
(`lod_pyramid_debug`, `adaptive_lod_debug`, `streaming_lod_scatter`).

## Background

The stories that built this code are GUP-076 (occlusion culling), GUP-077
(compute instance filtering), GUP-184 and GUP-235 (radix sort), GUP-222 and
GUP-223 (unified pipeline, Hi-Z early reject), GUP-234 (adaptive coverage
budget), GUP-256 (LOD pyramid) and GUP-257 (adaptive viewport renderer). See
also `docs/LOD_SYSTEM.md`.
