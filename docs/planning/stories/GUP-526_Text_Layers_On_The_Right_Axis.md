---
implementer: sonnet
implementer-reason:
  two setters and a per-layer choice of y scale threaded through the four places
  plot.rs hands text layers a scale, each named in Decisions; tests copy
  GUP-542's and GUP-555's
touches:
  [
    crates/gup-core/src/text_layer.rs,
    crates/gup-core/src/plot.rs,
    crates/gup-core/tests/text_layer.rs,
    crates/gup-core/tests/secondary_y.rs,
    tests/golden/gup_core/secondary_y.png,
  ]
---

# GUP-526: Text Layers on the Right Y Axis

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-26 (decision-complete rewrite 2026-10-31)

## Context

GUP-542's `TextLayer` always maps its y through the plot's left y scale. In
`plot.rs`, `layer.runs(&text, &**x, &**y, plot, &th)` is called around
line 2632. A dual-axis chart (`Plot::y_right`) draws a series against the right
axis, but a label for that series lands where the left scale puts the same
value. Nothing reports the error. A `Selection` binds to the right axis by
encoding through the right scale's handle (`right.encode(..)`). A text layer
takes plain closures, so it needs its own way to say which axis.

GUP-555 made text layers reserve room at the plot edges. It reports each layer's
reach and per-label edges along `[x, y]`, and `plot.rs` feeds them to the left y
axis only (`text_reach`, `label_room` and the `labels` candidates are
two-element arrays). A right-axis label must make room on the right axis
instead, or a label at the top of the right domain is clipped. Checked on main
on 2026-10-31: neither exists.

## Read First

- `crates/gup-core/src/text_layer.rs` lines 120–330 (`TextLayer` fields and
  setters, `y`, `y_key`, `within_y`), 394–418 (`check_axis`) and 440–630
  (`AnnotationLayer`: `runs`, `reach`, `edges`).
- `crates/gup-core/src/plot.rs`: `resolve_full`'s checks (lines 1866–1910); step
  5 from `let right_id` to the end of `solve_insets` (lines 2088–2320):
  `text_reach`, `text_over`, `label_room` and its `axes`, `widened`, and the
  `solve` block's `labels`; step 7's text-layer loop (lines 2626–2636).
- `crates/gup-core/tests/text_layer.rs`: the GUP-555 edge tests and the helper
  plots they build.
- `crates/gup-core/tests/secondary_y.rs` (the dual-axis golden, lines 1–190).

## User Story

> "As a visualization developer with a two-axis chart, I want to label the
> series drawn against the right axis, so that its labels sit on its points."

## Acceptance Criteria

### AC1: The API

- [x] `TextLayer::y_right(f)` and `TextLayer::y_right_key(key)` place labels
      through the plot's right y scale, like `y` and `y_key` on the left. The
      later of `y`, `y_key`, `y_right` and `y_right_key` wins. `within_y` spans
      are read on whichever axis y is on. Each setter has rustdoc, and `y_right`
      has a doctest: a two-axis plot whose label sits at `right.eval(v)` (± 0.5
      px) after resolve.
- [x] A layer on the right axis of a plot without one fails at resolve with
      `Error::config("text layer position", ..)`, whose message names
      `Plot::y_right`. Test in `tests/text_layer.rs`.
- [x] `y_right(..)` on a band or point right axis, and `y_right_key(..)` on a
      numeric one, give `check_axis`'s errors naming `y_right_key(..)` and
      `y_right(..)` respectively. Test in `tests/text_layer.rs`.

### AC2: Placement

- [x] Left `Linear` 0–10, right `Linear` 0–1000, a right-axis label at 500 with
      `dy(-6.0)`: its `PlacedText::run.at.y` equals `right.eval(500) - 6` within
      0.5 px. It is at least 20 px from the left scale's `eval(500) - 6`.
- [x] Seeded failure, recorded in the retrospective: passing the left scale to
      `runs` for every layer makes this test fail.

### AC3: Room at the edge (GUP-555's rule on the right axis)

- [x] Explicit right domain 0–1000, a label at 1000 anchored `BOTTOM`: its
      `PlacedText::bounds` lies inside `layout.plot`, so the label is drawn
      whole.
- [x] Automatic right domain, a label at the data maximum anchored `BOTTOM`:
      drawn whole, and the right axis widened (its top tick is at or above the
      data maximum).
- [x] With only left-axis labels, step 5 gives the same left and right insets as
      before. All of GUP-555's tests in `tests/text_layer.rs` pass unchanged.

### AC4: Golden (user-visible)

- [x] The golden test in `secondary_y.rs`, and not `fixture()`, which other
      tests share, adds
      `plot.text(months()).x(|m| m.n).y_right(|m| m.rain).text(|m| m.rain).anchor(Anchor::BOTTOM).dy(-4.0)`.
      The rainfall bars are the right-axis series. Re-bless only
      `secondary_y.png` and read it. Each value sits just above its bar's top,
      the 180 mm label is whole (the automatic right axis widened), and the
      temperature line still follows the left axis's ticks.
- [x] `git diff --stat tests/golden` lists only `secondary_y.png`.
- [x] `mask wasm-browser` passes (step 5 changed).

## Decisions

- **Setters** (`text_layer.rs`), mirroring `y` and `y_key`:

  ```rust
  pub fn y_right<D: ColumnValue>(&mut self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> &mut Self;
  pub fn y_right_key<K: DictionaryKey>(&mut self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> &mut Self;
  ```

  They set `self.y` as `y` and `y_key` do, plus a new field `y_side: YSide`
  (`enum YSide { Left, Right }`, private, default `Left`). `y` and `y_key` set
  `Left`. We chose this over `y_axis(YAxis::Right)`: the name matches
  `Plot::y_right`, and a newcomer who has written `.y(|r| r.temp)` guesses
  `.y_right(|r| r.rain)`.

- **Trait**: `AnnotationLayer` gains `fn on_right_axis(&self) -> bool` (not
  `y_right`, which would share a name with the setter). `runs`, `reach` and
  `edges` keep their signatures. The plot passes the right scale as their `y`
  argument for a right-axis layer. Inside `runs`, `check_axis(name, ..)` gets
  `"y_right"` when `y_side` is `Right`, so its messages name `y_right_key(..)` /
  `y_right(..)`.
- **The missing-axis error** is raised in `resolve_full` beside the z-order
  check, before step 5:

  ```rust
  Error::config(
      "text layer position",
      format!(
          "text layer {i} places its labels on the right y axis (`y_right(..)`), \
           but the plot has none; add one with `Plot::y_right(scale)`"
      ),
  )
  ```

- **Step 5** (`plot.rs`). Index 2 is the right axis everywhere, as in
  `mark_over`.
  - `text_reach` becomes `[f32; 3]`. A layer's `reach()[1]` goes to index 1, or
    to index 2 when `layer.on_right_axis()`.
  - `text_over[2]` is `text_reach[2].min(0.25 * lens[2])`, or 0.0 when the right
    axis reads keys or is absent.
  - `label_room` becomes `[(f32, f32); 3]`. The automatic-axis `axes` array
    gains the right scale, with `ends[2] = (bottom, top)` and the same
    `EdgeAxis` construction. Each layer calls
    `edges(&**x, y_of(layer), [axes[0], axes[1 or 2]], &mut tmp)` into a fresh
    `[Vec<Edge>; 2]`. Then `tmp[0]` extends `cands[0]` and `tmp[1]` extends
    `cands[1]` or `cands[2]`. The loop solving `label_room` runs over all three
    axes.
  - `widened[2]` uses `padded(mark_over[2].max(label_room[2].0))` and `….1` in
    place of the symmetric `padded(mark_over[2])`. It is identical when there
    are no right-axis labels.
  - The explicit `solve` block collects labels the same way into `cands[2]`.
  - `y_of(layer)` is
    `if layer.on_right_axis() { right.expect("checked in resolve_full") } else { y }`.
- **Step 7**: `layer.runs(&text, &**x, y_of(layer), plot, &th)`.
- **No change** to `Selection`, scales or `inset.rs`.

## Escalation Triggers

- Feeding right-axis edges into the explicit solve changes any left-axis inset
  in an existing test.
- GUP-555's arrays turn out to be indexed in a way that assumes two axes beyond
  the places named above (for example in `inset::edge_insets` callers outside
  step 5).
- The `secondary_y` golden needs more than the new labels to change.

## Out of Scope

- Text layers placed on a second x axis: there is none.
- Label halos and overlap culling: GUP-527.

## Technical Tasks

- [x] `YSide`, the setters and their docs and doctest;
      `AnnotationLayer::on_right_axis`.
- [x] `resolve_full`'s missing-axis error.
- [x] Step 5's three-axis arrays and step 7's scale choice.
- [x] Tests AC1–AC3 in `tests/text_layer.rs`; the seeded failure.
- [x] The `secondary_y` golden; `mask wasm-browser`.

## Dependencies

### Prerequisite Stories

- GUP-542 ✅: the text layer.
- GUP-555 ✅: label room at the plot edge (the arrays this extends).

## Testing Strategy

- **Unit tests**: none new; `text_layer::tests` must pass.
- **Integration tests**: AC1–AC3 in `crates/gup-core/tests/text_layer.rs`.
- **Visual validation**: `secondary_y.png` read by eye.
- **Performance**: unchanged (one more array slot).

## Success Metrics

- [x] A dual-axis chart labels both of its series with two `plot.text(..)`
      calls.

## Risk Assessment

- **Medium**: step 5 is dense, and GUP-555's arrays are easy to index wrongly.
  _Mitigation_: the edge tests for the left axis must pass unchanged, and AC3
  covers both right-axis policies.
- **Low**: `plot.rs` is a hot file. _Mitigation_: `touches` names it, so no
  other `plot.rs` story runs beside this one.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (`secondary_y.png`)
- [x] `mask wasm-browser` (step 5 changed)
- [x] WASM size: `mask wasm-size` scatter gz bytes (text layers are generic;
      expect at most 1,000 B gz)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Blocked (2026-11-01)

**Question**: the `secondary_y` golden now renders as the story describes, but
`svg_marks::assert_matches("gup_core/secondary_y", ..)` (the SVG-vs-GPU check in
the same test, outside `touches:`) fails: 2 of 20160 4x4 blocks differ by up to
ΔE 5.4, over `MAX_BLOCK_DE` = 5.0. Which way do we go?

**Evidence**: the new `180` label at x = 12 (anchor BOTTOM, wider than the 9 px
bar half-width) makes GUP-555 give the x axis a little room, so the temperature
line moves right by a fraction of a pixel (old vs new golden differ along the
line and at x ticks 3, 6, 9, 10, 12). The hot blocks are on the steep line
segments (around (185, 140) and (440, 350)), not on any label. This is the resvg
quarter-step vertical coverage the `MAX_BLOCK_DE` doc comment describes (4.7
measured over earlier goldens). The GPU golden itself is correct (read; the left
axis ticks 8..26 are unchanged, each rainfall value sits above its bar, 180 is
whole, the right axis ends near 190).

**Options**:

1. Raise `MAX_BLOCK_DE` to 5.5 with this measurement as the reason (a gate
   change, which I may not make).
2. Drop the label at x = 12 or give the bars a different width in the test (but
   the story fixes the label call and `fixture()`).
3. Shift the line by setting an explicit x domain in this test (changes the
   golden beyond the labels).

**Recommendation**: option 1.

Everything else is done and green: tests in `tests/text_layer.rs` (24 pass),
doctest, `mask all-fix`. Not yet run: `mask merge-check --dogfood`,
`mask wasm-browser`, wasm size.

**Resolved (2026-11-01)**: orchestrator recalibrated `MAX_BLOCK_DE` 5.0 -> 5.5
and re-enabled the heatmap SVG check (`heatmap_png.rs`'s `golden` no longer
takes an `svg` flag).

## Implementation Summary

- `TextLayer::y_right` / `y_right_key` (private `YSide`),
  `AnnotationLayer::on_right_axis`; `resolve_full` fails a right-axis layer on a
  plot without `Plot::y_right`; step 5 (`text_reach`, `text_over`, `label_room`,
  the `labels` candidates, `widened[2]`) is three-axis; step 7 picks the scale
  per layer. Files: `crates/gup-core/src/text_layer.rs`, `plot.rs`,
  `tests/text_layer.rs` (+7 tests, 24 total), `tests/secondary_y.rs`,
  `tests/golden/gup_core/secondary_y.png`.
- Gate change (approved): `MAX_BLOCK_DE` 5.5 in `tests/common/svg_marks.rs`;
  heatmap SVG check re-enabled.
- Golden (read): each rainfall value sits above its bar's top, "180" is whole at
  the plot edge, the right axis widens to about 190 with its top tick 180, the
  left axis still runs 8..26 and the temperature line follows it. The x axis
  gained a sub-pixel of room for the "180" label (GUP-555).
- `mask merge-check --dogfood`: all six steps PASS. WASM scatter 358,629 B gz
  (budget 354,964 +2%, pass). `cargo test --doc -p gup`: 56 pass. `wasm-browser`
  PASS.

## Retrospective

**Completed**: 2026-11-02

### Key Technical Learnings

#### Seeded failure (AC2)

- **Challenge**: prove the placement test bites.
- **Solution**: passing the left scale to `runs` for every layer made
  `a_right_axis_label_is_placed_through_the_right_scale`,
  `the_later_y_setter_wins` and the two explicit/automatic edge tests fail.
- **Pattern**: index a scale array by `on_right_axis()` in one place per step.

#### SVG-vs-GPU block tolerance

- **Challenge**: a label wider than a bar's overhang shifted the x range by a
  sub-pixel, moving the line's resvg coverage to ΔE 5.4 (limit 5.0).
- **Solution**: orchestrator recalibrated to 5.5. Max block ΔE of every case
  that runs `assert_matches`, measured on this branch before the change
  (heatmap's check re-enabled); the values do not change with the constant:

| case                 | max ΔE | case                 | max ΔE |
| -------------------- | ------ | -------------------- | ------ |
| secondary_y          | 5.38   | fallback_title       | 2.95   |
| segment_mark         | 4.64   | line_monotone        | 2.82   |
| heatmap              | 4.15   | time_years           | 2.58   |
| diverging_anomaly    | 3.67   | rect_bars            | 2.42   |
| line_series          | 3.61   | grouped_bars         | 2.41   |
| time_tick_hierarchy  | 2.95   | categorical_species  | 2.15   |
| scatter, grid        | 2.95   | line_steps           | 2.05   |
| heatmap_diverging    | 2.05   | theme_dark           | 2.01   |
| histogram            | 1.98   | band_months          | 1.95   |
| legend_two_layers    | 1.90   | categorical_nulls    | 1.84   |
| tick_format_date     | 1.78   | boxplot              | 1.62   |
| sqrt_radius          | 1.61   | theme_dark_brush     | 1.58   |
| text_annotations     | 1.55   | axis_titles          | 1.52   |
| tick_format_currency | 1.48   | tick_format_percent  | 1.48   |
| legend_guides        | 1.45   | area_mark            | 1.39   |
| symlog_signed        | 1.32   | composite_two_layers | 1.11   |
| brush                | 1.08   | brush_grey           | 0.76   |

Before (limit 5.0): only `secondary_y` failed. After (limit 5.5): all pass.
Nothing else sits between 4.7 and 5.5. The heatmap, said to reach 5.4 on its own
branch, measures 4.15 on current main.

- **Pattern**: a golden's SVG check can fail from a sub-pixel layout shift
  alone; measure every case before moving the limit.

### Architectural Decisions

#### Right-axis layers pass the right scale as `y`

- **Decision**: as specified; `runs`/`reach`/`edges` keep their signatures.
- **Trade-off**: `plot.rs` indexes `y_scales[side]`; `reach` is `[f32; 2]` and
  step 5 remaps its y entry to slot 2.

### Development Workflow Insights

- Worktree sandbox refuses compound shell with `sed -i` on variable paths and
  python heredocs mentioning paths; use Edit and plain commands.
- `cargo test` stops at the first failing test binary; use `--no-fail-fast`.

### Follow-up Stories

None. (Overlap of labels with lines in the golden, e.g. "48" and "90", is
GUP-527's halos.)
