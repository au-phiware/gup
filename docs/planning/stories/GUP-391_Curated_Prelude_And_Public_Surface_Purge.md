# GUP-391: Curated Prelude and Public Surface Purge

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: ⏸ Parked **Created**:
2026-10-04

**Parked 2026-10-04**: superseded by
[RFC-001](../rfcs/RFC-001_Core_Architecture.md) steps S7/S14. RFC-001 (Core
Architecture) is accepted and builds the new core in `crates/gup-core`; the old
`gup` crate path is frozen (no feature work) ahead of the S14 flip. A curated
prelude designed against today's `src/lib.rs` would be thrown away when
`gup-core` replaces it — the prelude is designed fresh as part of `gup-core`'s
own public-API work instead. Do not implement this story.

## Context

This story originally proposed replacing the 16 root glob re-exports in
`src/lib.rs` with a ~25-name curated prelude, deduplicating
`AccessorFunction`/`Margins`/`Color`/`Orientation`, and deleting silent no-op
builder methods (`anti_alias`, `point_budget`, `bar_width` in
`src/chart_builder/builders/scatter.rs` and `bar.rs`). That analysis remains
valid background for whoever designs `gup-core`'s prelude, but the mechanical
work of curating the **old** crate's exports is no longer worth doing on a
frozen path.

## Dependencies

### Prerequisite Stories

- GUP-389 (Delete unwired dead subsystems) and GUP-390 (Quarantine culling/LOD
  and park integration crates) remain valid prune work independent of this
  parking decision — see their own story files.

## Definition of Done

Not applicable — parked. Re-evaluate after RFC-001 S14, or treat its design
notes as input to `gup-core`'s own prelude story if one is written.
