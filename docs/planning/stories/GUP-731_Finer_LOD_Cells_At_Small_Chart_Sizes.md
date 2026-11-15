---
implementer: opus
implementer-reason:
  changes the tier rule or the per-chunk grid that every LOD frame draws, with a
  fidelity and GPU-time trade-off to measure
touches: [crates/gup-core/src/lod.rs, crates/gup-core/src/selection.rs]
---

# GUP-731: Finer LOD Cells at Small Chart Sizes

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 💡 New **Created**:
2026-11-15

## Context

GUP-716 ships `Lod::Auto` with the spike's tier rule, one tier finer than one
representative per pixel of area. Its fidelity (ΔE00 > 10 against `Lod::Off`, %
of the image / of the plot rect, `lod_spike --fidelity`) is near the spike's at
1920 × 1080 but not at small charts:

| scene                          | auto full     | spike B+1 full | auto zoom     | spike B+1 zoom |
| ------------------------------ | ------------- | -------------- | ------------- | -------------- |
| 10M, 1920 × 1080               | 0.727 / 0.788 | 0.580 / 0.628¹ | 1.088 / 1.171 | 1.224 / 1.236¹ |
| 1M, 1920 × 1080                | 0.807 / 0.874 | 0.567 / 0.614  | 0.000 / 0.000 | 0.000 / 0.000  |
| 1M, 400 × 300 (spike's walk)   | 1.760 / 2.510 | 1.284 / 1.831  | 2.211 / 3.115 | 1.348 / 1.900  |
| 1M, 400 × 300 (`tests/lod.rs`) | 1.098 / 1.566 | -              | -             | -              |

¹ GUP-631's table.

The cause is GUP-714's grid: per chunk, cells of a power of two of data units,
anchored so that appends leave built tiers valid. The grid spans up to twice the
chunk's extent on each axis, so at the same tier its cells are up to 4× the
spike's in area (at 400 × 300: 0.48 px² against 0.30; 10,763 representatives
against 16,912). Rounding the tier instead of flooring it (measured in GUP-716:
0.653% of the plot rect at 10M, 1080p, for about 40% more representatives)
leaves the 400 × 300 full view unchanged.

## User Story

> "As a developer of a small dashboard scatter of a million rows, I want the
> level-of-detail picture to be as close to the exact one as it is in a large
> chart."

## Acceptance Criteria

- [ ] Measure three options against GUP-716's rule at 400 × 300, 800 × 600 and
      1920 × 1080 (1M and 10M rows, full and 10× zoom): fidelity shares,
      representatives drawn, GPU median (three runs,
      `lod_spike --candidates     base,pyramid,auto`):
  1. cells that are not a power of two (a grid fitted to the chunk's extent,
     with a rule for appends);
  2. a size-aware tier bias (from the cell's area in pixels, e.g. the finest
     tier whose cells are at most 0.5 px²);
  3. rounding the tier (GUP-716's option B).
- [ ] Ship the option that brings the 400 × 300 full view within 0.3 points of
      the spike's design at that size, without raising the 10M 1080p GPU median
      above 8 ms, or report why none does.
- [ ] GUP-717's gates (re-derived per scene size) pass.

## Decisions

- GUP-714's invariant stays: built chunks' tiers stay valid as a layer grows.

## Escalation Triggers

- Option 1 needs tiers rebuilt on appends that do not touch a chunk.
- Every option costs more than 1.5× GUP-716's GPU time at 10M.

## Out of Scope

- Unsorted layers (GUP-720), size order (GUP-721).

## Technical Tasks

- [ ] The three variants behind a test seam; measurements; the chosen rule.

## Dependencies

### Prerequisite Stories

- GUP-716: `Selection::lod` and Per-Frame Tier Draws ✅.
- GUP-717: LOD Golden, Fidelity and Budgets (its per-size gates).

### Enables Stories

- None.

## Testing Strategy

- **Integration tests**: `tests/lod.rs` fidelity shares at the three sizes.
- **Performance**: `lod_spike` A/B, three runs, median.

## Success Metrics

- [ ] 400 × 300 full view within 0.3 points of the spike's design.

## Risk Assessment

- **Medium**: finer cells draw more representatives. _Mitigation_: the 8 ms gate
  at 10M.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
