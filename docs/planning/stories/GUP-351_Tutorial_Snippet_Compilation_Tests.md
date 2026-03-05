# GUP-351: Tutorial Snippet Compilation Tests

## Story Overview

**Initiative**: Documentation **Status**: ✅ Complete **Created**: 2025-07-15

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

- [x] A test harness extracts fenced Rust code blocks from tutorial Markdown
      files and verifies they compile (or are explicitly marked as pseudocode).
- [x] The test harness runs as part of `cargo test` or a dedicated `mask` task.
- [x] Code blocks marked with ` ```rust,ignore ` or ` ```rust,no_run ` are
      skipped.
- [x] At least the "Full Example" blocks from each tutorial pass the harness.
- [x] CI fails if a tutorial snippet stops compiling after an API change.

## Technical Tasks

- [x] Evaluate `skeptic`, `doc-comment`, or a custom `tests/doc_snippets/`
      approach for extracting and compiling Markdown code blocks.
- [x] Implement the chosen approach.
- [x] Add a `mask` task (e.g. `mask test-tutorials`) for convenience.
- [x] Verify all six tutorials pass the harness.

## Dependencies

### Prerequisite Stories

- GUP-281: Tutorial and Guide Suite ✅ — The tutorials to be tested.

## Testing Strategy

- Run the harness against all tutorials and verify pass/fail.
- Intentionally break a snippet to confirm the harness detects it.

## Success Metrics

- [x] All tutorial "Full Example" blocks compile via the test harness.
- [x] At least one intentional breakage is detected during development.

## Risk Assessment

- **Low**: Some tutorial snippets are intentionally incomplete (showing only
  relevant lines). The harness must handle this by only testing blocks that are
  marked as compilable.

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass
- [x] Story status updated to ✅ Complete

## Implementation Summary

### Approach Chosen

Used Rust's built-in `#[cfg(doctest)]` with `#[doc = include_str!(...)]` — the
modern, zero-dependency approach. Each tutorial Markdown file is included as a
doc comment on a dummy struct in `src/lib.rs`. Rustdoc extracts fenced code
blocks and compiles them as doctests. Tutorial 3 (Custom Shader Functions) is
tested via a separate integration test due to proc-macro `crate::` path
limitations.

### Key Files Changed

| File                                           | Change                                                |
| ---------------------------------------------- | ----------------------------------------------------- |
| `src/lib.rs`                                   | Added `#[cfg(doctest)]` module, proc-macro re-exports |
| `docs/tutorials/01_getting_started.md`         | Marked partial blocks `ignore`, full `no_run`         |
| `docs/tutorials/02_data_binding.md`            | Marked partial blocks `ignore`, full `no_run`         |
| `docs/tutorials/03_custom_shader_functions.md` | Marked all blocks `ignore` (proc-macro limitation)    |
| `docs/tutorials/04_interactions.md`            | Marked partial blocks `ignore`, full `no_run`         |
| `docs/tutorials/05_streaming_data.md`          | Marked partial blocks `ignore`, full `no_run`         |
| `docs/tutorials/06_custom_marks.md`            | Marked partial blocks `ignore`, full `no_run`         |
| `tests/tutorial_snippet_tests.rs`              | Integration test for Tutorial 3 Full Example          |
| `maskfile.md`                                  | Added `mask test-tutorials` task                      |

### Test Counts

- 5 doctest compilation checks (tutorials 1, 2, 4, 5, 6)
- 1 integration test (tutorial 3)
- 61 ignored blocks (partial snippets marked `rust,ignore`)
