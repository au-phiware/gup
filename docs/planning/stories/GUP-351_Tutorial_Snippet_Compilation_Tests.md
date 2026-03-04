# GUP-351: Tutorial Snippet Compilation Tests

## Story Overview

**Initiative**: Documentation **Status**: 📋 Planned **Created**: 2025-07-15

## Context

GUP-281 delivered six tutorials with code snippets derived from the existing
examples suite. However, there is no automated mechanism to verify that tutorial
code snippets continue to compile as the API evolves. Over time, refactors and
API changes may silently break tutorial examples, degrading the onboarding
experience for new developers.

## User Story

> "As a Gup maintainer, I want automated tests that verify tutorial code
> snippets compile so that I can catch documentation drift before it reaches
> users."

## Acceptance Criteria

- [ ] A test harness extracts fenced Rust code blocks from tutorial Markdown
      files and verifies they compile (or are explicitly marked as pseudocode).
- [ ] The test harness runs as part of `cargo test` or a dedicated `mask` task.
- [ ] Code blocks marked with ` ```rust,ignore ` or ` ```rust,no_run ` are
      skipped.
- [ ] At least the "Full Example" blocks from each tutorial pass the harness.
- [ ] CI fails if a tutorial snippet stops compiling after an API change.

## Technical Tasks

- [ ] Evaluate `skeptic`, `doc-comment`, or a custom `tests/doc_snippets/`
      approach for extracting and compiling Markdown code blocks.
- [ ] Implement the chosen approach.
- [ ] Add a `mask` task (e.g. `mask test-tutorials`) for convenience.
- [ ] Verify all six tutorials pass the harness.

## Dependencies

### Prerequisite Stories

- GUP-281: Tutorial and Guide Suite ✅ — The tutorials to be tested.

## Testing Strategy

- Run the harness against all tutorials and verify pass/fail.
- Intentionally break a snippet to confirm the harness detects it.

## Success Metrics

- [ ] All tutorial "Full Example" blocks compile via the test harness.
- [ ] At least one intentional breakage is detected during development.

## Risk Assessment

- **Low**: Some tutorial snippets are intentionally incomplete (showing only
  relevant lines). The harness must handle this by only testing blocks that are
  marked as compilable.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Story status updated to ✅ Complete
