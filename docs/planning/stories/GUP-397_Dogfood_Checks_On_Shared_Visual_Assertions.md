# GUP-397: Dogfood Checks on Shared Visual Assertions

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: 📋 Planned **Created**:
2026-10-04

## Context

GUP-394 brought the audit's dogfood crate into `dogfood/` and runs it in CI with
pixel checks. GUP-388 (visual regression harness) had not landed when GUP-394
was implemented, so GUP-394 wrote a small, Gup-independent measurement module,
`dogfood/src/pixels.rs`. It covers background detection, coverage, ink (text)
counts, hue counts, exact-colour counts, grey counts and horizontal bar runs.
The checks are driven by the manifest in `dogfood/src/suite.rs`, which expresses
regions as fractions of the image.

GUP-388 plans a target-agnostic assertion module that takes `RgbaImage` plus
`LayoutMetadata` (plot rect, expected text regions, expected colours). Once it
exists there will be two implementations of "is there text here" and "is this
colour present". These can drift apart, with different thresholds and different
notions of background. The strategic review's lesson ("ACs ticked while output
was broken") argues for a single, well-tested set of visual assertions used by
both the in-repo golden tests and the external-user suite.

The dogfood crate is detached from the workspace and may only depend on public
crates. So GUP-388's assertions must be reachable from an external crate, either
as a small published-in-workspace crate (e.g. `crates/gup-visual-assert`) or as
a public module that dogfood can depend on by path.

## User Story

> "As a maintainer, I want the dogfood suite and the golden-image tests to use
> the same visual assertions, so that a chart judged 'has a title' or 'uses the
> configured colour' means the same thing in both places."

## Acceptance Criteria

### AC1: One assertion implementation

- [ ] `dogfood/src/pixels.rs` is deleted, or reduced to thin glue. Every
      measurement it provided is served by GUP-388's assertion module, which
      gains the measurements dogfood needs if they are missing (hue-family
      count, grey count, horizontal run count).
- [ ] The dogfood crate depends on the assertion code by path without joining
      the parent workspace and without access to `pub(crate)` items in `gup`.

### AC2: Manifest unchanged in intent

- [ ] Every check in `dogfood/src/suite.rs` keeps its intent and its tracked
      `Gap` status. Thresholds may be re-derived, but no tracked gap flips from
      `XFAIL` to `PASS` because a measurement got looser. Any `XPASS` produced
      by the migration is investigated and explained in the retrospective.
- [ ] Where GUP-388's `LayoutMetadata` can express a region (plot rect, title
      band), the manifest uses it instead of hard-coded fractions.

### AC3: Suite still green in CI

- [ ] `./dogfood/run_all.sh` under Xvfb reports the same task outcomes as before
      the migration (record before/after summaries in the retro).

## Technical Tasks

- [ ] Decide where GUP-388's assertions live so an external crate can use them,
      and record the decision.
- [ ] Port each `Measure` variant in `suite.rs` to the shared assertions.
- [ ] Move `pixels.rs`'s synthetic-image unit tests into the shared module if
      they add coverage.
- [ ] Re-run the suite on GPU and forced lavapipe
      (`VK_ICD_FILENAMES=.../lvp_icd.x86_64.json LIBGL_ALWAYS_SOFTWARE=1`) and
      compare outcomes.

## Dependencies

### Prerequisite Stories

- GUP-394: Dogfood Suite in Repo and CI ✅
- GUP-388: Visual Regression Harness 📋

### Enables Stories

- None.

## Testing Strategy

- Unit tests for any measurement added to the shared module, using synthetic
  images.
- `cargo test --release --lib --bin dogfood_check` inside `dogfood/`.
- Full dogfood run under Xvfb, compared with the pre-migration summary.

## Success Metrics

- [ ] One implementation of each visual measurement in the repository.
- [ ] Dogfood task outcomes unchanged (or every change explained).

## Risk Assessment

- **Low**: Threshold differences between the two implementations cause spurious
  `FAIL`/`XPASS`. Mitigation: compare before/after summaries and re-derive
  thresholds from fresh renders, never by loosening a tracked gap.
- **Low**: Exposing GUP-388's assertions to an external crate adds a small
  public surface. Mitigation: keep it in a separate dev-only crate, not in the
  `gup` prelude.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] `cargo test -- --test-threads=1` passes and the dogfood suite passes under
      Xvfb.
- [ ] Lint and format clean: `mask all-fix`.
- [ ] Story status updated to ✅ Complete in story file and INDEX.md.
- [ ] Retrospective added to the story document.
