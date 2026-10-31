---
implementer: sonnet
implementer-reason:
  one role-to-features rule applied at the three places tick labels are measured
  or laid out, an SVG style attribute and a margin test with a known failure
  mode
touches:
  [
    crates/gup-core/src/scene.rs,
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/svg.rs,
    tests/golden/,
  ]
---

# GUP-579: Tabular Figures for Tick Labels

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-31

## Context

Inter's default digits are proportional. In the Regular subset "1" advances 833
font units and "0" advances 1,292, so "1111" is 3,332 units wide and "1000" is
4,709 (kerned). Right-aligned y tick labels are therefore ragged. Worse, the y
margin is the widest label's width (`plot.rs`, `width_of` in the `layout`
closure). A zoom that turns "1000 … 2000" into "1100 … 1200" moves the plot rect
even though every label still has four digits. The strategic review's T4a asked
for "a neutral UI face with tabular figures".

[GUP-405](GUP-405_Gup_Text_OpenType_Shaping.md) gives `gup-text` a `Features`
set on `Run` and `measure_with`, where `Features::TNUM` applies Inter's GSUB
`tnum`. With it, all ten digits advance 1,328 units in Regular and 1,325 in
SemiBold. This story turns it on for tick labels and nothing else, and tells SVG
viewers to do the same.

## Read First

- `crates/gup-core/src/scene.rs` lines 307–352: `TextRole`, `TextRun`,
  `layout_run`.
- `crates/gup-core/src/plot.rs`: the `layout` closure's `width_of` (around
  line 2019) and the `thin` closure's `collides` (around line 2335), which
  measure tick labels. Also the three `TextRole::TickLabel` runs (around lines
  2475–2525).
- `crates/gup-core/src/svg.rs` lines 340–380: the `ItemKind::Text` branch of
  `SvgTarget::write`.
- GUP-405's Decisions: `Features`, `TextSystem::measure_with`.
- `crates/gup-core/tests/svg.rs` (how SVG text is asserted) and
  `crates/gup-core/tests/tick_format.rs` (a plot with formatted y ticks).

## User Story

> "As a Gup user reading a chart, I want tick labels set in tabular figures, so
> that numbers line up in columns and the axis margin stays put while I zoom."

## Acceptance Criteria

### AC1: Tick labels are tabular, everything else is not

- [ ] `TextRole::features(self) -> gup_text::Features` (`pub(crate)`) returns
      `KERN | TNUM` for `TickLabel` and `KERN` for every other role.
      `TextRun::layout_run` uses it, so drawing and `ink_bounds` agree.
- [ ] Both tick-label measuring sites in `plot.rs` (`width_of` and `collides`)
      call
      `text.measure_with(label, th.tick_size.0, weight, TextRole::TickLabel.features())`.
      A grep for `measure_weight(` in `plot.rs` finds no tick-label call left.
- [ ] Unit test in `scene.rs`: a `TickLabel` run "1111" and a `TickLabel` run
      "1000" have equal `ink_bounds` widths ±1 px, and as `Title` runs they
      differ by at least 5 px at 16 px.

### AC2: The margin holds steady

- [ ] Integration test `crates/gup-core/tests/tick_format.rs`
      `tabular_ticks_keep_the_y_margin`: a scatter with
      `y_tick_format(|v| format!("{v:.0}"))`, resolved at 480×320 with these
      explicit y domains in turn: 1000–2000, 1100–1200, 1110–1120, 1500–1600,
      1900–2000. `layout.plot.left()` is identical for all five.
- [ ] The test also asserts that it would catch the bug: for the same labels,
      `measure_with(.., Features::KERN)` widths differ by at least 3 px between
      the widest and narrowest label set. Seeded failure, recorded in the
      retrospective: with `TickLabel` mapped to `KERN` alone, the test fails.

### AC3: SVG

- [ ] `SvgTarget` writes ` style="font-variant-numeric:tabular-nums"` on every
      `<text>` whose run's role is `TickLabel`, and on no other. A test in
      `crates/gup-core/tests/svg.rs` checks a plot's SVG: every y tick label
      element has it, and the title element does not.

### AC4: Goldens (user-visible), with a cap

- [ ] Before blessing, every failure is a `golden` check failure (no structural
      or `svg_marks` failure). After `GUP_BLESS=1`,
      `git diff --stat     tests/golden` lists only goldens that draw numeric
      tick labels. Give the count in the retrospective.
- [ ] Read `scatter.png` and `secondary_y.png` before and after. The y labels
      align digit for digit on the right edge. The left margin moves by at most
      3 px.

### AC5: Size

- [ ] `mask wasm-size`: scatter grows by at most 1,500 B gz. Report the figures.
- [ ] `mask wasm-browser` passes (margins move).

## Decisions

- **Only `TextRole::TickLabel` gets `TNUM`.** Titles, subtitles, axis titles,
  legends, tooltips and annotations keep proportional digits: they are prose,
  not columns. Band and point axis labels are `TickLabel` too. Digits in a
  category name become tabular, which is harmless.
- **The rule lives in one place**:
  `impl TextRole { pub(crate) fn features(self) -> gup_text::Features }` in
  `scene.rs`. Measuring sites ask the role, never spell the bits out.
- **Vertical metrics are unaffected**: `measure("0", tick_size)` for `x_label_h`
  (cap height and descent) stays as is.
- **SVG** uses a `style` attribute, not a presentation attribute:
  `font-variant-numeric` is a CSS property that SVG 1.1 viewers apply only
  through `style`. resvg 0.45 ignores it. That is fine: the parity check masks
  text regions.
- **The test plot** in AC2 uses explicit domains and a plain `{v:.0}` format, so
  every tick label is four digits and nothing else changes the margin.

## Escalation Triggers

- The AC2 margin still moves with `TNUM` on: report the five tick label lists
  and their `measure_with` widths. A second cause, such as kerning across a
  minus sign or a held margin, needs a decision.
- A golden fails a structural or parity check, or a non-golden test's pixel
  value changes by more than 3 px.

## Out of Scope

- Tabular figures for value labels (GUP-544's `value_labels`) and colour-ramp
  legend labels (`guide.rs`, role `Legend`). Both are numbers, but neither is
  measured as an axis margin. A follow-up can add a role if they need it.
- `font-feature-settings` for other features, and changing the subset.

## Technical Tasks

- [ ] `TextRole::features` and `layout_run`; scene unit test.
- [ ] The two `plot.rs` measuring sites.
- [ ] SVG style attribute and its test.
- [ ] `tabular_ticks_keep_the_y_margin`, with the seeded failure.
- [ ] Re-bless, read the goldens, `mask wasm-size`, `mask wasm-browser`.

## Dependencies

### Prerequisite Stories

- GUP-405: gup-text kerning and glyph-index layout 📋. Provides `Features::TNUM`
  and `measure_with`.

### Enables Stories

- RFC-001 S7 (layout and guide emitters): stable label widths make the
  tick-density rule easier to hold.

## Testing Strategy

- **Unit tests**: `TextRole::features`; tabular widths in `scene.rs`.
- **Integration tests**: the five-domain margin test; the SVG style test.
- **Visual validation**: two goldens read by eye.
- **Performance**: none beyond GUP-405's (one hash lookup per glyph).

## Success Metrics

- [ ] The y margin of a four-digit axis is the same at every zoom.
- [ ] WASM grows by at most 1,500 B gz.

## Risk Assessment

- **Low**: tabular digits are wider than proportional "1", so some margins grow
  by a few pixels. _Mitigation_: the goldens record it. The cap is 3 px.
- **Medium**: the re-bless conflicts with any parallel golden-changing story.
  _Mitigation_: `touches` lists `tests/golden/`.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (two goldens)
- [ ] `mask wasm-browser` (layout changes)
- [ ] WASM size: `mask wasm-size` scatter gz bytes
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
