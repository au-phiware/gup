# GUP-428: Prune Superseded Shared Units

## Story Overview

**Initiative**: Strategic Review 2026-10 (T0 guardrails) **Status**: 💡 New
**Created**: 2026-10-16

## Context

[GUP-427](GUP-427_Bound_Build_Directory_Disk_Growth_Per_Commit.md) prunes the
shared build directory after every Rust hook run: units and incremental caches
of checkouts that no longer exist (recorded by
`scripts/rustc_workspace_wrapper.sh`), superseded and idle incremental sessions,
and caches over a size cap. It never touches units without an owner record:
dependencies, clippy's member units (clippy bypasses the wrapper), and anything
built before GUP-427. Those are shared and usually live, but some become dead
and stay forever:

- dependency units of versions a `Cargo.lock` change replaced;
- every unit after a toolchain bump (rustc's version is in every hash);
- clippy units of test or example targets that were deleted or renamed;
- units of checkouts removed before GUP-427 landed.

## User Story

> "As an orchestrator, I want dependency and clippy units nobody can use any
> more to be pruned too, so that the build directory does not grow with every
> dependency or toolchain update."

## Acceptance Criteria

- [ ] Measure what each case leaves: a `Cargo.lock` bump of one dependency, a
      toolchain bump (rebuild with a different rustc), a deleted test target.
- [ ] A liveness rule for unowned units that never deletes a unit a live
      checkout's next build uses (for example: a toolchain-stale rule from the
      fingerprint JSON's `rustc` field, plus age since last use recorded by a
      wrapper stamp of `--extern` paths).
- [ ] `scripts/test_prune_build_dir.sh` gains cases for each rule, including a
      warm checkout staying `Fresh`.

## Technical Tasks

- [ ] Read cargo's fingerprint JSON for the `rustc` hash and decide which value
      is current (newest unit, or `cargo -vV`).
- [ ] Decide whether `build.rustc-wrapper` (all units, not only members) may
      stamp use without changing artifact hashes; measure its overhead.
- [ ] Extend `scripts/prune_build_dir.sh` and its header comment.

## Dependencies

### Prerequisite Stories

- GUP-427 ✅: the prune, its locking and the seeded check.

### Coordinate With

- GUP-413 💡: per-checkout clippy units would also need an owner.

## Testing Strategy

- **Script test**: seeded cases in `scripts/test_prune_build_dir.sh`.
- **Measurement**: before/after `du` of a real dependency and toolchain bump.

## Success Metrics

- [ ] After a toolchain bump and one hook run, the build directory holds one
      toolchain's units.

## Risk Assessment

- **Medium**: deleting a live dependency unit forces a rebuild of every
  dependent (minutes). Mitigation: conservative rules, the warm-checkout case.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] Lint and format clean: `mask all-check`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
