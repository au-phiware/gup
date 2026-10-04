# GUP-387: Fix Pre-existing Markdown Lint Violations Blocking the Pre-commit Hook

## Story Overview

**Initiative**: Developer Experience **Status**: ✅ Complete (Superseded)
**Created**: 2026-10-04 **Closed**: 2026-10-04

**Closed as superseded**: resolved by the 2026-10-04 merge of the
backlog-hygiene branch, which ran `prettier --write` and fixed the outstanding
`mdl` violations across `docs/`. Verified on `main` on 2026-10-04:
`mdl --git-recurse .` and `prettier --check "**/*.md"` both exit 0. See
[STRATEGIC_REVIEW_2026-10.md](../STRATEGIC_REVIEW_2026-10.md#backlog-triage).

## Context

The pre-commit hook runs `mask all-check`, which includes
`prettier --check "**/*.md" && mdl --git-recurse .`. On current `main` this step
fails because of violations in files that most commits never touch:

- `mdl` reports MD028 (blank line inside blockquote), MD013 (line length) and
  MD038 (spaces inside code spans) in about 17 story documents, including
  GUP-248, GUP-249, GUP-259, GUP-260, GUP-263, GUP-266 to GUP-272, GUP-280,
  GUP-281, GUP-282, GUP-282A and GUP-310.
- `prettier --check` fails on recently completed story documents (for example
  GUP-364, GUP-365 and GUP-382) that were committed without formatting.

The hook therefore fails for every commit, including docs-only commits, so
contributors and agents have to pass `--no-verify`. That bypasses the Rust
checks as well. GUP-377 had to do this for every commit.

## User Story

> "As a contributor, I want the pre-commit hook to pass on a clean checkout of
> `main`, so that a hook failure always means my change introduced a problem."

## Acceptance Criteria

- [x] `mdl --git-recurse .` exits 0 on `main`.
- [x] `prettier --check "**/*.md"` exits 0 on `main`.
- [x] `mask all-check` passes on a clean checkout.
- [x] Story-worker guidance notes that `mask all-fix` must be run before
      committing story documents, so violations do not come back (already
      present in `.github/agents/story-worker.md`).

## Technical Tasks

- [x] Run `prettier --write` across `docs/`.
- [x] Fix MD028 by joining blockquote paragraphs or removing the blank line
      between consecutive quotes.
- [x] Fix MD013 and MD038 by hand where prettier cannot.
- [x] If a rule conflicts with prettier, consider excluding it in `.mdl.style`
      and give the reason in a comment.

## Dependencies

### Prerequisite Stories

- None.

## Testing Strategy

- Run `mask all-check` and confirm it exits 0.

## Success Metrics

- [x] No `--no-verify` commits needed for docs-only changes.

## Retrospective

Closed without separate implementation: the 2026-10-04 backlog-hygiene branch
(merged the same day, ahead of this story being picked up) ran
`prettier --write` across `docs/` and fixed the pre-existing `mdl` violations as
part of its own cleanup. Re-verified independently while triaging the October
2026 strategic review: `mdl --git-recurse .` and `prettier --check "**/*.md"`
both exit 0 on `main`. No further work needed.

## Risk Assessment

- **Low**: These are formatting-only changes. Concurrent story workers editing
  the same documents may get merge conflicts, so land this when few stories are
  in flight.

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] `mask all-check` passes
- [x] Story status updated to ✅ Complete (Superseded)
