---
implementer: sonnet
implementer-reason: a small option on one script, with seeded cases
touches: [scripts/release_check.sh, scripts/test_release_check.sh]
---

# GUP-606: Release Check Reads Workflow Runs for a Branch

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: ✅
Complete (2026-11-12) **Created**: 2026-11-04

## Context

GUP-604's `ci` row reads the runs of `origin/main`, and `clean-tree` fails when
HEAD is not on or above it. Run on a worktree branch (as a story worker does),
the table says nothing about the branch's own runs. Do this only if
`release-check` runs on branches matter before a merge.

## User Story

> "As a worker, I want `release-check --ci-ref HEAD` to read the runs of my
> pushed commit, so that I know my branch is green before the merge."

## Acceptance Criteria

- [x] `scripts/release_check.sh --ci-ref REF` reads the runs of `REF`'s commit
      (default `origin/main`), SKIP when it was not pushed.
- [x] A seeded case with the fake `gh` covers it.

## Dependencies

### Prerequisite Stories

- GUP-604 ✅

## Testing Strategy

Seeded: `scripts/test_release_check.sh`.

## Risk Assessment

- **Low**: a read-only option on the release tooling.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] `mask all-fix` clean
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `scripts/release_check.sh --ci-ref REF` (default `origin/main`): the `ci` row
  reads the workflow runs of `REF`'s commit. A commit that no remote-tracking
  ref contains was never pushed, so the row is SKIP (no `gh run list`). An
  unknown ref is a FAIL.
- `docs/RELEASING.md` checklist item 1 documents the option.
- `scripts/test_release_check.sh`: three seeded cases on the fake `gh`:
  `ci_ref_pushed` (a pushed `origin/topic` is read, origin/main is not),
  `ci_ref_unpushed` (HEAD not pushed: SKIP, no `run list`), `ci_ref_unknown`
  (FAIL).
- Validation: `scripts/test_release_check.sh` OK; `mask all-check` exit 0, every
  line checked.

## Retrospective

**Completed**: 2026-11-12

### Key Technical Learnings

#### "Pushed" means a remote-tracking ref contains the commit

- **Challenge**: The story says SKIP when the commit was never pushed, and
  checking that against the remote needs the network.
- **Solution**: `for-each-ref --contains SHA refs/remotes/` answers it offline.
  A push updates the remote-tracking ref, so a pushed branch passes, and a
  local-only commit SKIPs before any `gh` call.
- **Pattern**: Use local remote-tracking state for "was it pushed" checks in
  read-only release tooling.

### Architectural Decisions

#### Keep the clean-tree check on origin/main

- **Decision**: `--ci-ref` changes only the `ci` row. `clean-tree` still
  requires HEAD to be on or above origin/main.
- **Reasoning**: The story scopes the option to the runs; changing the ancestry
  check would alter a second gate.
- **Trade-off**: On a branch behind origin/main, `clean-tree` still fails even
  with `--ci-ref HEAD`.
- **Future**: Could accept the same ref there if branch runs matter more than
  the ancestry rule.

### Development Workflow Insights

- The pre-commit hook's Markdown check failed on a story header that prettier
  re-wraps. Run `prettier --write` before committing story edits.
- A pushed commit with no runs yet still FAILs the row (`no run on <sha>`), as
  the default does; left unchanged.

### Follow-up Stories

None.
