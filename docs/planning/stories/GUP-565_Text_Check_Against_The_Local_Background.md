---
implementer: sonnet
implementer-reason:
  one helper in the harness's text check, with the sampling rule and the tests
  fixed in Decisions
touches: []
---

# GUP-565: Text Check Against the Local Background

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-26

## Context

The visual-regression harness's `text_present` check counts, in each expected
text region, the pixels that look like the region's text colour blended over the
chart's background (`check_text_present` and `is_glyph`,
`crates/gup-visual-regression/src/checks.rs` lines 141–300). The background is
one colour for the whole image (`LayoutMetadata::background`).

Text drawn on a coloured fill fails it. GUP-544's stacked bars print dark or
white labels centred on orange, blue and green segments; against white, those
glyphs are not on the white-to-text blend line, and the check finds 2–7 glyph
pixels where it needs 8. GUP-544's golden adapter therefore drops every
Annotation text from the metadata and counts white or `#111111` pixels in each
label box itself (`crates/gup-core/tests/bar_value_labels.rs` lines 292–324).
That is an adapter working around the harness. The same will hit heatmap cell
labels, area labels and tooltips over a panel.

This story makes the check measure each region against the colour actually
behind its text, sampled from the image, and restores the stacked golden's
standard check.

## Read First

- `crates/gup-visual-regression/src/checks.rs`: `Tolerances` (lines 98–135),
  `is_ink` and `is_glyph` (lines 137–169), `check_text_present` (lines 248–300),
  and the text tests from line 425 (`synthetic`,
  `mark_fill_flooding_a_text_region_is_not_text`).
- `crates/gup-visual-regression/src/layout.rs` lines 118–182 (`TextRegion`,
  `LayoutMetadata::background`, `in_guide`).
- `crates/gup-core/tests/bar_value_labels.rs` lines 292–380 (`golden` and the
  stacked golden test).

## User Story

> "As a developer adding labels on coloured marks, I want the standard text
> check to find my text, so that I do not write a custom pixel count per
> golden."

## Acceptance Criteria

### AC1: The local background

- [ ] A private `fn local_background(img, region, layout, tol) -> Rgba8` in
      `checks.rs` implements the rule in Decisions, and `check_text_present`
      passes its result to `is_glyph` in place of `layout.background`.
- [ ] The failure message names the background it used when it differs from the
      layout's (e.g. `… (5 glyph px of #111111 on #e69f00)`).

### AC2: Unit tests

- [ ] New test: on the synthetic image, fill the title region with `#e69f00` and
      draw dark (`#111111`) glyph pixels (a few solid and a few 50% blends)
      inside it; with the region's colour `#111111` the check passes.
- [ ] New test: the same orange region with no glyph pixels fails and names the
      region.
- [ ] New test: white text (`#ffffff`) on `#0072b2` passes (the white-text case
      of contrast-coloured labels).
- [ ] Every existing test in `checks.rs` and `harness.rs` passes unchanged,
      including `mark_fill_flooding_a_text_region_is_not_text` (a flooded region
      with no text still fails: its local background is the flood).

### AC3: The stacked golden uses the standard check

- [ ] In `tests/bar_value_labels.rs`, `golden` loses its `in_fill` parameter,
      the `plain.texts.retain(..)` filter and the hand-written pixel count; the
      stacked golden passes with every Annotation text in the metadata.
- [ ] The golden image itself is unchanged (no re-bless).

### AC4: Nothing else changes status

- [ ] `scripts/with_cargo_env.sh cargo test -p gup-core -- --test-threads=1`,
      `cargo test -p gup-visual-regression`, `cargo test --test tutorials` and
      `./dogfood/run_all.sh` pass;
      `tests/visual_regression/expected_failures.toml` stays without entries.

## Decisions

- **The rule**: over the region's pixel span, skipping pixels in a guide region
  (`layout.in_guide`) and pixels within `tol.text_blend_delta_e` of
  `region.color.over(layout.background)` (the text's own solid pixels), the
  local background is the most frequent exact `Rgba8` value. Ties go to
  `layout.background` if it is among them, else to the smallest `(r, g, b, a)`.
  A region with no such pixel uses `layout.background`. Flat fills and
  backgrounds are exact colours, while anti-aliased glyph edges are spread over
  many values, so the mode is the fill.
- **Automatic, no new field**: `TextRegion` does not gain a background field.
  Every region already declares its text colour, and the image says what is
  behind it; an adapter-supplied colour would be one more thing to get wrong.
- **Tolerances unchanged**: the fix is the reference colour, not a looser
  threshold.

## Escalation Triggers

- Any existing harness case changes status (a PASS becomes FAIL, or an XFAIL
  would become XPASS).
- A region whose text is absent passes because its mode is a mark colour close
  to the text colour.

## Out of Scope

- The other checks (`marks_present`, `marks_confined`, `not_blank`) keep the
  global background.
- Dogfood's own pixel checks (`gup_visual_regression::measure`).

## Technical Tasks

- [ ] `local_background` and its use in `check_text_present`; message.
- [ ] AC2 unit tests.
- [ ] Simplify `golden` in `tests/bar_value_labels.rs`.
- [ ] Run AC4's suites.

## Dependencies

### Prerequisite Stories

- GUP-544: Bar Value Labels ✅ — the stacked golden and its adapter workaround.

### Enables Stories

- Labels on fills in later charts (heatmap cell labels, deferred in N2) get the
  standard check for free.

## Testing Strategy

- **Unit tests**: AC2 on synthetic images.
- **Integration tests**: the stacked golden with full metadata; all suites using
  the harness.

## Success Metrics

- [ ] The stacked golden's adapter has no special case.
- [ ] Zero status changes elsewhere.

## Risk Assessment

- **Low**: a tiny region where glyph pixels outnumber the background could pick
  an anti-aliased colour as its background. _Mitigation_: full-coverage glyph
  pixels are excluded before the mode is taken, and AC4 runs every suite.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`, plus
      `scripts/with_cargo_env.sh cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not applicable (the harness changes, no
      image does)
- [ ] `mask wasm-browser` not needed: no layout, input or wasm code changes
- [ ] WASM size: unchanged, no library code change (test harness only)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
