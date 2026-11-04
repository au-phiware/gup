---
implementer: sonnet
implementer-reason: a small change to one task and its documentation
touches: [maskfile.md, scripts/]
---

# GUP-603: Publish Check Private Package Directory

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 📋 Planned
**Created**: 2026-11-04

## Context

On 2026-11-04 `mask publish-check` passed, then failed twice in the same
checkout with `no hash listed for gup-core-macros v0.1.0` (cargo's
`failed to verify package tarball`).
`cargo publish --workspace --dry-run --locked` passed with a private
`CARGO_BUILD_BUILD_DIR`, and CI passes. The cause is probably the shared
`$CARGO_BUILD_BUILD_DIR/package/tmp-registry` left by another checkout's dry run
of the same version with different contents. `mask release-check` must be
reliable on a machine with several checkouts.

## User Story

> "As the maintainer, I want the publish dry run to be reliable in a shared
> build directory, so that release day has no surprises."

## Acceptance Criteria

- [ ] `mask publish-check` passes twice in a row, and after another checkout ran
      it, with `CARGO_BUILD_BUILD_DIR` shared.
- [ ] A test or a documented rule shows the cause is removed.

## Dependencies

### Prerequisite Stories

- GUP-598 ✅

## Testing Strategy

Run `mask publish-check` from two checkouts alternately.

## Risk Assessment

- **Low**: a change confined to the release tooling.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-fix` clean
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
