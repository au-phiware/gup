---
implementer: sonnet
implementer-reason: a small option on one script, with seeded cases
touches: [scripts/release_check.sh, scripts/test_release_check.sh]
---

# GUP-606: Release Check Reads Workflow Runs for a Branch

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 💡 New
**Created**: 2026-11-04

## Context

GUP-604's `ci` row reads the runs of `origin/main`, and `clean-tree` fails when
HEAD is not on or above it. Run on a worktree branch (as a story worker does),
the table says nothing about the branch's own runs. Do this only if
`release-check` runs on branches matter before a merge.

## User Story

> "As a worker, I want `release-check --ci-ref HEAD` to read the runs of my
> pushed commit, so that I know my branch is green before the merge."

## Acceptance Criteria

- [ ] `scripts/release_check.sh --ci-ref REF` reads the runs of `REF`'s commit
      (default `origin/main`), SKIP when it was not pushed.
- [ ] A seeded case with the fake `gh` covers it.

## Dependencies

### Prerequisite Stories

- GUP-604 ✅

## Testing Strategy

Seeded: `scripts/test_release_check.sh`.

## Risk Assessment

- **Low**: a read-only option on the release tooling.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-fix` clean
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
