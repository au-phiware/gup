# GUP-557: Keep SvgTarget Text Out of the WASM Build

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 💡
New **Created**: 2026-11-01

## Context

GUP-580 found that `SvgTarget::write` is linked into the WASM scatter although
the `svg` feature is off, so SVG text code costs WASM bytes (+1,329 B gz for
fallback fonts alone before gating). GUP-580 gated only the fallback code.

## User Story

> "As a web user, I want SVG export code excluded from my WASM bundle unless I
> use it, so that charts load faster."

## Acceptance Criteria

- [ ] Find what retains `SvgTarget::write` in the scatter harness
      (`mask wasm-attrib`).
- [ ] If it can go without an API change, `wasm.scatter.gz_bytes` drops and the
      orchestrator re-records the budget.
- [ ] The fallback code may then be ungated.

## Technical Tasks

- [ ] Attribute the retention; gate it or make it unreachable; measure.

## Dependencies

- GUP-580 (complete).

## Testing Strategy

- `mask wasm-size` parent and after; `cargo test -p gup-core`.

## Success Metrics

- [ ] Measured gz reduction reported.

## Risk Assessment

- **Low**: gating changes the guides-only behaviour.

## Definition of Done

- [ ] ACs met, `mask all-fix` clean, INDEX updated.
