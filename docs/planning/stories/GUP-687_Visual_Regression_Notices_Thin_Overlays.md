---
implementer: opus
implementer-reason:
  the rule is simple to write, but its threshold must hold on Windows WARP and
  macOS Metal, which cannot be measured locally; choosing it from CI evidence
  and a seeded failure is a tolerance judgement
touches:
  [
    crates/gup-visual-regression/src/diff.rs,
    crates/gup-visual-regression/src/golden.rs,
    tests/golden/README.md,
  ]
---

# GUP-687: The Golden Comparison Notices A Thin Overlay

## Story Overview

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: 🚧 In
Progress **Created**: 2026-11-08

## Context

GUP-676 added a 16 px square ring (1 px ink, 1 px edge) to every hovered render.
Four existing goldens gained it (`tooltip`, `hello_world`, `theme_dark`,
`tour/08_tooltip`) and all four still passed: the worker had to bless the whole
suite to find which changed.

Correction to the premise: the golden comparison has no block step. It is
`perceptual_diff` in `crates/gup-visual-regression/src/diff.rs`: a pixel differs
when its CIEDE2000 distance exceeds `pixel_delta_e` 3.0, and a render matches
when at most `max_differing_fraction` 0.0025 (0.25%) of pixels differ.
`MAX_BLOCK_DE` 5.5 and the 4 by 4 `blocks()` averaging belong to a different
check, the SVG-versus-GPU comparison in
`crates/gup-core/tests/common/svg_marks.rs`, and are not involved. The ring
slips through because it is small in count, not averaged: a 16 px ring has at
most 16 \* 4 - 4 = 60 edge pixels plus about 52 ink pixels, about 112 pixels,
and 0.25% of a 640 by 400 render is 640. A one-pixel hairline feature (a rule, a
focus ring, a thin border) of any contrast is invisible to the fraction rule
until it has more than 0.25% of the area.

Where goldens run: Linux in `tests.yml`; Windows WARP and macOS Metal in
`platforms.yml` (`cargo test -p gup --tests`: the tour, tutorial and gallery
goldens, not `tests/golden/gup_core`). Antialiased edges and glyph pixels differ
a little across those backends, which is why the fraction rule is loose.

## Read First

- `crates/gup-visual-regression/src/diff.rs` (whole file; 208 lines).
- `crates/gup-visual-regression/src/golden.rs` lines 100 to 190 (`check`, the
  `BlessReason::Changed` branch) and 330 to 360 (the tolerance tests).
- `tests/golden/README.md` "Perceptual-diff tolerance".
- `.github/workflows/platforms.yml` (steps that run goldens and upload
  `windows-golden-diffs` and `macos-golden-diffs`).
- GUP-676 Implementation Summary ("Existing goldens that changed").
- `crates/gup-core/tests/common/svg_marks.rs` lines 92 to 119, only to see that
  its block constants are a different check.

## User Story

> "As a maintainer, I want a golden test to fail when a thin, high-contrast
> feature appears or disappears, so that I notice which images a change touched
> without blessing the suite."

## Acceptance Criteria

### AC1: Measure first (the spike)

- [ ] Add `DiffStats::strong_pixels` (count of pixels with ΔE above
      `DiffTolerance::strong_delta_e`, default 12.0) and print it in `Display`
      (not yet asserted). Run the whole golden suite on Linux and record, per
      golden, `strong_pixels` for a fresh render against its checked-in golden:
      expected 0 everywhere. List any non-zero and why.
- [ ] Push the branch to CI with the report only (no assertion) and read
      `strong_pixels` for every tour, tutorial and gallery golden from the
      Platforms output (the `--nocapture` log) on Windows WARP and macOS Metal.
      Record the maximum per platform in the story. The orchestrator pushes; the
      implementer reads the logs.

### AC2: The rule

- [ ] Decision rule from AC1: let `M` be the largest `strong_pixels` seen on any
      platform. Set `max_strong_pixels` to `max(2 * M, 8)` rounded up to a
      multiple of 4. Then assert in `DiffStats::within`:
      `strong_pixels <= tol.max_strong_pixels` in addition to the fraction rule.
      If `M` exceeds 24, stop and escalate: the rule would not catch a 16 px
      ring (about 100 strong pixels at black on white) with margin.
- [ ] `strong_delta_e` 12.0 is a starting value: it must be above the ΔE of a
      half-covered antialiased edge pixel against background that the platforms
      differ on (measured in AC1) and below the ΔE of the ring's ink or edge
      against a typical background (about 40 for dark ink on white). If AC1
      shows platform pixels above 12, raise it to the lowest value that makes
      `M` at most 24 and record that.
- [ ] `DiffTolerance` gains `strong_delta_e: f32` and
      `max_strong_pixels:     usize` and keeps `Default`; the existing
      struct-literal uses in `golden.rs` tests, `window.rs`, `svg.rs` and
      `svg_marks.rs` are updated (`..DiffTolerance::default()`).
- [ ] The failure message for a render rejected by the strong rule names the
      count and says whether the fraction rule alone would have passed.

### AC3: A seeded failure

- [ ] New unit test in `diff.rs`: a 640 by 400 white render with a 16 px square
      ring (1 px black ink inside 1 px light-grey edge, drawn as `GUP-676` draws
      it) added fails `within` with `perceptual_diff`'s default tolerance, and
      the same render without the ring passes. A second test: the same scene
      with every edge pixel moved by one level of coverage (±8 per channel,
      antialiasing-like noise) still passes.
- [ ] Test through `GoldenStore::check`: a golden of the plain render and the
      ringed render as `actual` returns the mismatch error, with a diff image
      that marks the ring in magenta.
- [ ] The existing suite passes on Linux (`cargo test -- --test-threads=1`).

### AC4: Cross-platform

- [ ] The Platforms workflow passes (Windows WARP and macOS Metal) with the rule
      on. CI must be watched: after the orchestrator pushes, the implementer or
      orchestrator reads both jobs before the story is closed. If either fails
      on a golden whose render has no thin feature, that is evidence the
      threshold is wrong: raise `max_strong_pixels` or `strong_delta_e` per
      AC2's rule, never loosen the fraction rule, and say so in the
      retrospective.
- [ ] `tests/golden/README.md` "Perceptual-diff tolerance" documents the new
      rule, both constants and how they were chosen (with the AC1 maxima).

## Decisions

- **Add a count-of-strong-pixels rule beside the fraction rule**, rather than
  lowering `max_differing_fraction` (which fails on antialiasing noise across
  backends and glyph differences) or adding a block mean (which averages thin
  features away, the opposite of the need). It is the smallest change: one more
  statistic in the loop `perceptual_diff` already runs.
- **Absolute count, not fraction**, because the feature to catch is small in
  count whatever the image size.
- **Threshold chosen from measured platform maxima** (AC2's rule), not from
  taste.
- **Public API change is internal**: `gup-visual-regression` is a workspace test
  crate; no CHANGELOG entry.
- **Out of the golden path**: the SVG-versus-GPU block check in `svg_marks.rs`
  is not changed.

## Escalation Triggers

- `M` over 24 (platforms disagree by more strong pixels than a ring has).
- A Linux golden has non-zero `strong_pixels` against its own checked-in file
  (nondeterministic rendering).
- Any golden's blessing is needed to pass on Linux (the rule found a real
  change: report which and why, do not bless silently).
- CI cannot be read (no access to the Platforms logs).
- The seeded ring does not fail with the chosen constants.

## Out of Scope

- Changing the fraction rule, `pixel_delta_e`, or the SVG block comparison.
- Making goldens catch changes that are invisible (sub-threshold colour drift).
- A per-golden override mechanism (add one only if AC4 shows a platform needs it
  and report it).

## Technical Tasks

- [ ] `strong_pixels` stat and report; run on Linux; push for CI; read logs.
- [ ] Constants, `within`, message; update struct literals.
- [ ] Seeded tests; README.
- [ ] Watch Platforms; record maxima and the final constants in the story.

## Dependencies

### Prerequisite Stories

- GUP-388: Visual Regression Harness ✅ — the crate.
- GUP-676: Hovered Row Is Highlighted ✅ — the evidence.

## Testing Strategy

- **Unit tests**: seeded ring fails, AA-like noise passes, identical passes.
- **Integration tests**: `GoldenStore::check` mismatch; whole suite on Linux.
- **Cross-platform**: Platforms workflow on WARP and Metal.

## Success Metrics

- [ ] A 16 px ring added to any golden fails the test.
- [ ] No golden fails on any of the three platforms without a real change.

## Risk Assessment

- **Medium**: an unmeasured platform difference (a WARP glyph pixel) produces a
  strong pixel and fails CI. _Mitigation_: AC1 measures first; the headroom is 2
  times the maximum.
- **Low**: the constants are too tight for later goldens. _Mitigation_: the
  failure message names the count; the README explains how to re-derive.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: the seeded diff image read
- [ ] `mask wasm-browser`: not needed, test crate only
- [ ] WASM size: unchanged: no library code change
- [ ] Platforms workflow green on Windows and macOS (watched)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
