---
implementer: sonnet
implementer-reason:
  the pixel measure's algorithm, thresholds and negative control are fixed in
  Decisions; dogfood-only change following `Measure::Blobs`
---

# GUP-478: Style-Agnostic Tooltip Check in Dogfood Task 3

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-19

## Context

Dogfood task 3b (`t3_scatter_window`) hovers a point and checks that a tooltip
appears. The check is `Measure::Ink { min_px: 6000 }` on the tooltip's region:
6,000 pixels darker than the background. It was calibrated on the old egui
popup, which was dark. `gup-core`'s default tooltip is a white box with a grey
border and `#333333` text, which has far fewer dark pixels. GUP-465 made the
check pass by giving the task a dark tooltip theme
(`tooltip_background: #1b1b1b`, `tooltip_text: WHITE`), so the default tooltip a
user sees is not the one being checked.

GUP-465's retrospective names the lesson: "a threshold calibrated on an accident
is a poor bar. Prefer measures of intent." The intent here is "a tooltip box is
drawn and shows the datum". This story adds a measure that recognises a tooltip
box of any colours: a plain panel that hides the marks under it and holds lines
of text in a contrasting colour. t3 then drops its dark theme and checks the
default tooltip.

## Read First

- `dogfood/src/suite.rs`: `Measure` (lines 57–110), `Measure::evaluate` (lines
  147–260), `fn t3_png` (line 708) and `fn t3_window` (lines 908–957).
- `dogfood/src/pixels.rs` (whole: `background`, `count_ink`, `blob_centroids`,
  `luma`, `hue_sat`, and the synthetic-image unit tests).
- `dogfood/src/bin/t3_scatter_window.rs` (whole, 74 lines).
- `crates/gup-core/src/tooltip.rs` lines 1–95 (box, border, padding, pitch).
- [GUP-465](GUP-465_RFC_001_S10f_Dogfood_On_Gup_Core.md) "Decisions made during
  implementation" (the dark-theme stopgap) and its region table.

## User Story

> "As the project, I want the dogfood suite to check the tooltip a user actually
> gets, so that a default-style change cannot pass or fail the check by
> accident."

## Acceptance Criteria

### AC1: The measure

- [ ] `Measure::Tooltip { region: Region, lines: usize }` exists, with the doc
      comment in Decisions, and is evaluated by a new
      `pixels::tooltip_panel(img, region) -> Panel` (Decisions).
- [ ] Unit tests in `pixels.rs`, on synthetic images, give:
  - a white panel with a 1 px `#999999` border and five `#333333` text bars
    (with one intermediate-grey antialiasing column each), on a background of
    coloured dots: pass with `lines: 5`;
  - the same with a `#1b1b1b` panel and white bars: pass;
  - the same white panel with two bars and `lines: 5`: fail;
  - the region filled with dots of three hues on white, no panel: fail;
  - a white panel whose interior is 10% covered by coloured dots: fail.
- [ ] The detail string reads like
      `fill #ffffff, text #333333, 98.7% panel, 5 lines (need >= 95%, >= 5)`.

### AC2: t3 checks the default tooltip

- [ ] `t3_scatter_window.rs` no longer sets a theme (the `.theme(..)` call and
      its comment are deleted).
- [ ] `fn t3_window`'s "hover tooltip shown at the hovered point" check becomes
      `Measure::Tooltip { region: Region::new(0.595, 0.34, 0.705, 0.475), lines: 5 }`
      (the same region; five lines because the tooltip text has five fields).
- [ ] `xvfb-run -a ./dogfood/run_all.sh` reports no FAIL and no XPASS; paste the
      t3 window lines into the Implementation Summary. Read `t3_window.png`: a
      white tooltip box with grey border and five dark lines of text.

### AC3: Negative control

- [ ] The same measure on `t3_scatter.png` (the same chart, 1000×700, no hover)
      at the same region fails. Run it with `dogfood_check` or a one-off test,
      and paste its detail string into the Implementation Summary.
- [ ] If the region in AC2 does not match the tooltip box in a fresh render
      (`Layout::tooltip`), say so and use the Layout's box, converted to
      fractions as GUP-465 did; list old → new.

### AC4: Docs

- [ ] `dogfood/README.md` "Task 3" says the window check recognises a tooltip
      panel of any style, and no longer mentions the dark theme.

## Decisions

- **Doc comment**: "A tooltip box fills `region`: a panel of one fill colour,
  any colour, holding at least `lines` lines of text in a contrasting colour,
  and hiding the marks under it."
- **Algorithm** (`pixels::tooltip_panel`), on `region` inset by 2 px per side
  (skips a border of any colour and sub-pixel slop in the region):
  1. `fill` = the most common exact RGB in the interior.
  2. Text pixels: `|luma(p) − luma(fill)| >= 90` (the contrast `count_ink`
     uses). None: the result fails with "no text".
  3. `ink` = the most common exact RGB among the text pixels.
  4. Panel pixels: within 8 (max channel) of the segment from `fill` to `ink`,
     that is, the fill, the text colour, or an antialiased blend of the two.
     `panel_fraction` = panel pixels / interior pixels.
  5. `lines`: scan interior rows; a row is inked when it has at least 2 text
     pixels; a line is a run of at least 3 consecutive inked rows; runs are
     separated by at least one row that is not inked.
  6. Returns
     `Panel { fill: [u8; 3], ink: [u8; 3], panel_fraction: f32, lines: usize }`.
- **Pass rule**: `panel_fraction >= 0.95` and `lines >= lines`. 0.95 allows
  stray pixels at the inset edge; data marks under a translucent or misplaced
  box are off the fill–ink segment and fail it.
- **Why five lines separate**: the tooltip's pitch is `round(1.4 × 12) = 17` px
  and a line's ink is about 12 px tall (`tooltip.rs`), so lines have gaps of
  about 5 px.
- **Kept**: `Measure::Ink` (tick-label and title checks use it).

## Escalation Triggers

- The default tooltip fails the new check at the GUP-465 region and at the
  Layout's tooltip box.
- The negative control (AC3) passes.
- The lines of the default tooltip merge into fewer than five bands.

## Out of Scope

- Grey unbrushed rows for task 4: GUP-477.
- Changing `gup-core`'s tooltip style: not this story.

## Technical Tasks

- [ ] `pixels::tooltip_panel`, `Panel`, the unit tests.
- [ ] `Measure::Tooltip` and its evaluation and detail string.
- [ ] t3 binary and manifest; README; run the suite and the negative control.

## Dependencies

### Prerequisite Stories

- GUP-465: RFC-001 S10f Dogfood on gup-core ✅ — t3 on gup-core.

### Enables Stories

- None.

## Testing Strategy

- **Unit tests**: the five synthetic cases in AC1
  (`cargo test --manifest-path dogfood/Cargo.toml`).
- **End to end**: `xvfb-run -a ./dogfood/run_all.sh` (locally with
  `env -u WAYLAND_DISPLAY` and lavapipe, per GUP-465).
- **Visual validation**: read `t3_window.png`.

## Success Metrics

- [ ] Task 3b checks the default tooltip, with no theme override.
- [ ] The check fails on the same chart without a tooltip.

## Risk Assessment

- **Low**: the 2 px inset leaves the border of a thick-bordered theme inside the
  interior. _Mitigation_: such a border is a third colour off the segment; at 1
  px per side of a 110×94 box it costs under 5%, inside the 0.95 rule.
- **Low**: trybuild `.stderr` churn: none, as this story changes no gup-core
  code.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `cargo test --manifest-path dogfood/Cargo.toml` passes
- [ ] `xvfb-run -a ./dogfood/run_all.sh` passes (no FAIL, no XPASS)
- [ ] Lint and format clean: `mask all-fix`
- [ ] `mask wasm-browser`: not needed (no layout, input or wasm change); say so
      in the Implementation Summary
- [ ] WASM size: no library code changes; record "unchanged" in the
      Implementation Summary
- [ ] Rendered output verified by eye: `t3_window.png`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
