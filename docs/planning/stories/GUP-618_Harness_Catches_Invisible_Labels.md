---
implementer: sonnet
implementer-reason:
  one added condition in the harness's text check, with a measurement that sets
  its threshold by a stated rule and a seeded failing case
touches:
  [
    crates/gup-visual-regression/src/checks.rs,
    crates/gup-visual-regression/src/harness.rs,
  ]
---

# GUP-618: Harness Catches Invisible Labels

## Story Overview

**Initiative**: Polish after 0.1 **Status**: ✅ Complete (2026-11-04)
**Created**: 2026-11-04

## Context

The visual-regression harness's `text_present` check
(`crates/gup-visual-regression/src/checks.rs`, `check_text_present` line 291)
counts, in each expected text region, pixels that look like the region's text
colour blended over the region's _local background_. GUP-565 made that
background a sample: `local_background` (line 249) takes the most frequent exact
colour in the region, **skipping pixels close to the text's own solid colour**,
and falls back to the layout background when nothing is left.

That skip has a blind spot. A label drawn in the colour of what is behind it
(white text on a white bar, a contrast rule that picks the wrong side, a value
label in the series colour placed inside its own bar) is invisible, yet:

- every pixel of the region is "close to the text colour", so all are skipped;
- the local background falls back to the layout background (white);
- the fill pixels then look exactly like solid text over white, so the region
  counts hundreds of "glyph" pixels and passes.

So the harness passes a chart whose labels nobody can read.

## Read First

- `crates/gup-visual-regression/src/checks.rs` lines 100–170 (`Tolerances`,
  `is_glyph`) and 240–340 (`local_background`, `check_text_present`), and the
  text tests from line 470.
- `crates/gup-visual-regression/src/layout.rs` lines 115–131 (`TextRegion`).

## User Story

> "As a maintainer, I want the harness to fail a label drawn in its own
> background's colour, so that invisible text cannot pass CI."

## Acceptance Criteria

### AC1: Measure the margin on real outputs

- [x] For every text region of every golden test that runs the harness's checks
      (the `gup` crate's `tests/`, `gup-core`'s `tests/`, and dogfood's
      `run_all.sh`), compute `share` = the fraction of the region's non-guide
      pixels whose exact colour equals the region's most frequent exact colour
      _and_ lies within `text_blend_delta_e` of the solid text colour (text
      colour over the layout background). Record the maximum `share` over all
      passing regions and which region has it, in the retrospective. (A
      temporary `eprintln!` behind an environment variable is fine; remove it
      after.)

### AC2: The check

- [x] `check_text_present` fails a region when its most frequent exact colour
      (over non-guide pixels, nothing skipped) is within `text_blend_delta_e` of
      the solid text colour and its `share` exceeds the threshold `t` chosen by
      the rule in Decisions. The failure detail names the region and says the
      text is drawn in its background's colour, with both colours in hex.
- [x] Seeded unit tests in `checks.rs`: (a) a filled rectangle with a region
      over it whose text colour equals the fill fails with the new message; (b)
      the same with a contrasting text colour drawn as glyph-like strokes
      passes; (c) a region on the plain background with correct text passes.
- [x] Every existing suite still passes: `cargo test -p gup-visual-regression`,
      `cargo test -- --test-threads=1`,
      `cargo test -p gup-core -- --test-threads=1`, and `./dogfood/run_all.sh`
      (10 PASS).

### AC3: Negative control on a real chart

- [x] A test (in `gup-core`'s `tests/bar_value_labels.rs`) renders a vertical
      bar chart with `value_labels()` and a stack whose segment labels are
      forced to the segment's fill colour through `plot_mut()` (or any public
      route that draws a label in its fill colour), runs `check_text_present`,
      and asserts it fails. If no public route can draw such a label, build the
      image by hand instead (draw the chart, then paint the label regions with
      the fill) and say so in the test's doc comment.

## Decisions

- **Rule for `t`**: `t = min(0.9, (max_share + 1.0) / 2.0)`, with `max_share`
  from AC1. If `max_share ≥ 0.8`, stop (Escalation Triggers): the margin is too
  thin for a share test.
- **Where**: inside `check_text_present`, per region, before counting glyph
  pixels; no new `Check` variant (adding one would change `Check::ALL`, the
  expected-failure files and every report).
- **Why the dominant colour**: a correctly drawn label's glyph box is mostly
  background; its exact solid text colour appears only in glyph interiors, and
  antialiased edges spread over many colours, so the most frequent exact colour
  is the background. An invisible label's region is one colour.

## Escalation Triggers

- `max_share ≥ 0.8` in AC1 (bold large text or tight glyph boxes fill most of a
  region).
- An existing golden fails the new check: that is either a real invisible label
  (report it, with the image) or a flaw in the rule.

## Out of Scope

- A WCAG contrast-ratio requirement for all text (a separate check, if wanted).
- Tooltip text (not a text region today).

## Technical Tasks

- [x] AC1 measurement across the suites.
- [x] The condition and its message; unit tests.
- [x] The negative control on a real chart.

## Dependencies

### Prerequisite Stories

- GUP-565: Text Check Against the Local Background ✅.
- GUP-397: Dogfood Checks on Shared Visual Assertions ✅.

## Testing Strategy

- **Unit tests**: the three seeded cases.
- **Integration tests**: all golden suites and dogfood unchanged; AC3's real
  chart fails.

## Success Metrics

- [x] An invisible label fails `text_present` with a message that names it.

## Risk Assessment

- **Medium**: a share threshold is a heuristic. _Mitigation_: it is set from the
  measured margin on every real region, and the trigger stops a thin one.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: the commands of AC2
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, no image changes
- [x] `mask wasm-browser`: not needed, the harness does not run in the browser
- [x] WASM size: unchanged: no library code change
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `check_text_present` (`crates/gup-visual-regression/src/checks.rs`) now fails
  a region whose most frequent exact colour (nothing skipped, guides excluded)
  is within `text_blend_delta_e` of the solid text colour and covers more than
  `INVISIBLE_TEXT_SHARE` = 0.5 of its pixels. The message names the region and
  says the text is drawn in its background's colour, with hex colours.
- Seeded unit tests: a label in its fill colour fails; contrasting strokes on a
  fill and correct text on the plain background pass.
- `crates/gup-core/tests/bar_value_labels.rs`: a stacked bar chart with
  `value_labels()` passes, then its five labels are painted in their segment
  fills (by hand: no public route sets a value label's colour) and the check
  fails.
- Results: `cargo test -p gup` 101 pass; `cargo test -p gup-core` 807 pass;
  `cargo test -p gup-visual-regression` 58 pass; dogfood 10 PASS; no golden
  changed.

## Retrospective

**Completed**: 2026-11-04

### Key Technical Learnings

#### AC1 measurement

- **Challenge**: pick the threshold from real data.
- **Solution**: a temporary `eprintln!` per region over
  `cargo test -p gup-core --tests` (1458 regions), `cargo test -p gup --tests`
  (687) and dogfood. The maximum `share` over passing regions is **0.0**: in no
  real region is the most frequent exact colour near the text's own colour (dark
  text on white, white text on dark fills). So
  `t = min(0.9, (0 + 1) / 2) = 0.5`. The margin is the whole range, well below
  the 0.8 trigger. The probe was removed.
- **Pattern**: dogfood's runner has no text-region check (it printed no
  regions), so it neither feeds nor is affected by this check.

### Architectural Decisions

#### Threshold as a private constant

- **Decision**: `INVISIBLE_TEXT_SHARE` is a private const, not a `Tolerances`
  field.
- **Reasoning**: the story adds a condition to one check, not a knob; a public
  field is API surface.
- **Trade-off**: not tunable per case, which suits "never loosen tolerances".

### Development Workflow Insights

- The worktree started behind main and lacked the story; fast-forwarded.
- The pre-commit hook checks `cargo fmt` on the touched crates; run
  `cargo fmt -p ...` before committing.

### Follow-up Stories

None written (GUP-653 and GUP-654 unused). A contrast-ratio check is the story's
Out of Scope; dogfood has no text regions to check yet.
