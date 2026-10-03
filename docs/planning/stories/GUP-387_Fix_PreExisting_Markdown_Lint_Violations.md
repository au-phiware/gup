# GUP-387: Fix Pre-existing Markdown Lint Violations Blocking the Pre-commit Hook

## Story Overview

**Initiative**: Developer Experience **Status**: 📋 Planned **Created**:
2026-10-04

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

- [ ] `mdl --git-recurse .` exits 0 on `main`.
- [ ] `prettier --check "**/*.md"` exits 0 on `main`.
- [ ] `mask all-check` passes on a clean checkout.
- [ ] Story-worker guidance notes that `mask all-fix` must be run before
      committing story documents, so violations do not come back.

## Technical Tasks

- [ ] Run `prettier --write` across `docs/`.
- [ ] Fix MD028 by joining blockquote paragraphs or removing the blank line
      between consecutive quotes.
- [ ] Fix MD013 and MD038 by hand where prettier cannot.
- [ ] If a rule conflicts with prettier, consider excluding it in `.mdl.style`
      and give the reason in a comment.

## Dependencies

### Prerequisite Stories

- None.

## Testing Strategy

- Run `mask all-check` and confirm it exits 0.

## Success Metrics

- [ ] No `--no-verify` commits needed for docs-only changes.

## Risk Assessment

- **Low**: These are formatting-only changes. Concurrent story workers editing
  the same documents may get merge conflicts, so land this when few stories are
  in flight.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] `mask all-check` passes
- [ ] Story status updated to ✅ Complete
