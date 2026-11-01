---
implementer: sonnet
implementer-reason: shell work in one script, extends GUP-428's rule 5
touches: [scripts/]
---

# GUP-577: The Cap Evicts Unowned Incremental Caches

## Story Overview

**Initiative**: Strategic Review 2026-11 (Track N0) **Status**: 💡 New
**Created**: 2026-11-01

## Context

[GUP-428](GUP-428_Prune_Superseded_Shared_Units.md) rule 5 evicts units only.
Incremental caches (`<profile>/incremental/`) count toward `du` but are bounded
only by rules 2-4 (8 GB per profile, 3 days idle). Cargo builds path
dependencies incrementally too, so the caches of unowned crates (a path
dependency outside the workspace) are not units and survive rule 5. On the live
directory the report shows 8.7 GB of them, so a dry run at a tiny cap ends
"still over the cap".

## User Story

> "As the orchestrator, I want the cap to be a cap on the whole directory, so
> that incremental caches cannot keep it over."

## Acceptance Criteria

- [ ] After units, rule 5 evicts incremental crate caches, least recently
      compiled first, under the same keep window and locks, until at or under
      the cap.
- [ ] A seeded case with a path dependency outside the workspace stays under the
      cap (GUP-428's `make_depw` can use a path crate again).

## Dependencies

- GUP-428 ✅.

## Testing Strategy

`scripts/test_prune_build_dir.sh`.

## Risk Assessment

- **Low**: an evicted cache costs one non-incremental compile.

## Definition of Done

- [ ] Acceptance Criteria satisfied
- [ ] `mask all-check` passes
- [ ] Status updated in story and INDEX.md
- [ ] Retrospective added
