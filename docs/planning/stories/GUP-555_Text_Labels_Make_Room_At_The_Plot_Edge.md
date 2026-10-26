---
implementer: sonnet
implementer-reason:
  design settled in Decisions; extends GUP-528's inset solver and GUP-529's
  widening with per-side label extents measured once and cached
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/plot/inset.rs,
    crates/gup-core/src/text_layer.rs,
    crates/gup-core/src/scale/mod.rs,
  ]
---

# GUP-555: Text Labels Make Room at the Plot Edge

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-26

## Context

GUP-542 added `Plot::text` and `TextLayer`. A text layer adds no overhang, so
GUP-528's edge insets (explicit domains) and GUP-529's widening (automatic
domains) ignore it, and a label near the plot edge is clipped. The golden
`tests/golden/gup_core/text_annotations.png` shows it: Sydney, a point at the
right end of the explicit x domain, is labelled "Sydn".

GUP-542's retrospective says making room "would mean measuring labels before
step 5". That is a fixed-point problem only if the label's size depends on the
ranges, and it does not. A label's ink box relative to its anchor point (text,
size, weight, anchor, `dx`/`dy`) is a constant number of pixels, like a disc's
radius. Only where the label sits (its normalized position `t` on the axis)
depends on the domain, and GUP-528's solver already works in `t`.

Two details matter. First, labels are one-sided: a label above a point needs
room at the top only. Today's widening is symmetric (`padded(over)` at both
ends), so value labels above bars (GUP-544) would push an automatic y domain
below 0 and lift the bars off the axis. Second, labels must not double the cost
of a full resolve, which GUP-542 measured at +0.55 ms per 1,000 labels.

This story makes every text layer report per-side extents. Explicit axes take
them into GUP-528's solver, automatic axes into a data-aware, per-end `widen`.
Marks without labels behave exactly as today.

## Read First

- `crates/gup-core/src/plot.rs` `resolve_full` step 5 (lines 1882–2082: `over`,
  `ranges`, `padded`, `widened`, the `solve` block, `solve_insets`) and step 7's
  text-layer loop (lines 2290–2298).
- `crates/gup-core/src/plot.rs` `Plot::text` rustdoc (lines 656–689) and
  `PlacedText` (lines 40–47).
- `crates/gup-core/src/plot/inset.rs` (whole, 111 lines: `EdgeAxis`,
  `edge_insets`, tests).
- `crates/gup-core/src/text_layer.rs`: `Evaluated` and `TextLayer` fields (lines
  70–114), `evaluate` (300–321), `AnnotationLayer` and `runs` (372–481).
- `crates/gup-core/src/scale/mod.rs`: `DynPositionScale::widen` (line 381) and
  its impl (lines 504–522).
- `crates/gup-text/src/layout.rs` lines 188–262 (`ink_bounds`, `origin`, `walk`:
  the origin is rounded to whole pixels before glyphs are placed).
- `crates/gup-core/tests/text_layer.rs` lines 320–480 (the edge-clip test, the
  `annotated()` plot and `text_annotations_golden`) and 524–580 (the cost test).
- For reference only, not edited: `selection.rs` `Overhang` (lines 820–830) and
  `Layer::edge_marks` (lines 904–914).

## User Story

> "As a visualization developer, I want a label next to a point on the plot's
> edge to be drawn whole, so that I need not pad domains by hand or move labels
> inward to read them."

## Acceptance Criteria

### AC1: Cached label boxes (measured once)

- [x] `TextLayer` caches each label's ink box at anchor `(0, 0)` and baseline
      origin (Decisions, "Measurement"). It measures again only after a setter
      runs or when the effective size changes (a theme `tick_size` change with
      no `.size(..)`).
- [x] Unit test in `text_layer.rs`: for `Inter` Regular and SemiBold, the nine
      `Anchor` constants, sizes 11, 12 and 16, the texts `"Sydney"`,
      `"Alice Springs"` and `"−0.5"`, and 50 positions with pseudo-random
      fractional parts in 0..800, the translated cached box equals
      `Font::ink_bounds` of the run at that position (`f32` `==` on all four
      fields). **Decision rule**: if this holds, `runs` uses the translated box
      and calls no `ink_bounds`. If not, `runs` keeps calling `ink_bounds` as
      today, the cache serves only the extents (AC2), and the retrospective
      gives the largest difference found.

### AC2: Explicit domains inset per side

- [x] `inset::edge_insets` takes `(t, near, far)` candidates (Decisions). Its
      unit tests pass with the old cases written as `(t, h, h)`, plus
      `(1.0, 0.0, 5.0) → (0.0, 5.0)`, `(0.0, 0.0, 5.0) → (0.0, 0.0)` and
      `(1.0, 5.0, -3.0) → (0.0, 0.0)` (len 100, pad 4, cap 10). The
      `no_candidate_is_clipped` property test draws `near` and `far`
      independently in `0..cap` and checks each against its own end.
- [x] Integration test (explicit x and y domains `0..10`, one disc of radius 4
      at `(10, 10)`): adding a text layer that labels it with `Anchor::BOTTOM`,
      `dy(-6.0)` moves the pixel of `y = 10` down by at least the label's ink
      height + 6 − 4 px (± 0.5), and leaves the pixel of `y = 0` unchanged (to
      0.01 px). With `Anchor::LEFT`, `dx(4.0)` instead, the pixel of `x = 10`
      moves left and that of `x = 0` does not change.
- [x] In both cases the label's `PlacedText::bounds` is its whole ink box. Its
      width is that of the same label placed mid-plot (± 0.01), and it lies
      inside `Layout::plot`.

### AC3: Automatic domains widen per side

- [x] `DynPositionScale::widen` takes `near` and `far` (Decisions). With no text
      layers every plot's widened domains are the same as before (all existing
      tests and goldens pass unchanged, except AC5's).
- [x] Integration test (automatic x and y, three discs whose largest y is a
      round number that `nice` keeps, labels `Anchor::BOTTOM`, `dy(-6.0)`):
      every label's bounds lie inside `Layout::plot` and are its whole ink box
      (width as in AC2). The lowest disc's centre is the same number of pixels
      above `plot.bottom()` as in the same plot without the text layer (to 0.01
      px): the bottom gets no room for labels that hang above their points. The
      highest disc's centre is lower than without the text layer.
- [x] A label whose point is not in the domain adds no room. Integration test:
      on explicit domains, adding a row whose x is outside the domain (like
      Auckland) to the text layer leaves both axes' ranges unchanged (to 0.01
      px).

### AC4: Band and point axes, and the cap

- [x] Labels placed with `x_key` on a `Band` x axis change neither the x range
      nor the band positions. Test: the same plot with and without the text
      layer gives equal `key_position` for every key (to 0.01 px).
- [x] Cap: a 200-character label at the explicit x maximum, anchored
      `Anchor::LEFT`, makes the right inset at most `0.25 × plot.width` (+ pad +
      0.01). The label is still drawn, clipped to the plot rect.

### AC5: The golden and the docs

- [x] `text_annotations_golden` asserts Sydney whole: its bounds' centre x is
      within 1.0 px of `run.at.x`, and `bounds.right() < layout.plot.right()`.
      The "cut at the right edge" assertion and the `cities()` comment go. The
      `VISIBLE` list is unchanged.
- [x] The golden is re-blessed (`GUP_BLESS=1`) and read by eye: all six city
      labels are whole and centred over their discs, "Sydney" ends inside the
      plot rect, and the threshold label still ends 4 px left of the line's end.
      No label touches a tick label (existing assertion). The SVG raster check
      (`svg_marks::assert_matches`) passes.
- [x] `labels_on_the_plot_edge_are_clipped_to_the_plot_rect` passes. Its labels'
      anchors lie just outside the domain, so they are still clipped. Its
      "Labels do not change the layout" comment becomes "Labels outside the
      domain make no room". If the test fails because the probe's ranges differ
      from the real plot's, move both labels' anchors 0.5 data units further
      outside the domain, so that only part of each label's ink is still inside.
- [x] The `Plot::text` rustdoc paragraph and the `text_layer.rs` module doc say
      what Decisions "Docs" says. The `Plot::text` doctest (an external crate)
      uses Sydney at `lon: 150.0` and asserts, from `resolve`, that every
      `TextRole::Annotation` label's bounds centre x is within 1.5 px of its
      `run.at.x`. Check once that this assertion fails on the code before this
      story, and say so in the retrospective.

### AC6: Cost

- [x] `resolve_cost_of_a_thousand_labels` in release
      (`cargo test -p gup-core --release --test text_layer resolve_cost -- --nocapture`),
      run three times with `uptime` load below 6: the median `delta` is at most
      0.8 ms. GUP-542 measured 0.55 ms.
- [x] `a_cached_resolve_does_no_text_layer_work` passes unchanged: a cached
      resolve measures nothing and computes no extents.

## Decisions

- **Per-side extents, symmetric cap.** Each label gives, per numeric axis, a
  `near` and a `far` extent in px (x: left, right; y: bottom, top), measured
  from the label's data position:
  - `near_x = −(dx + rel.left) + 1`, `far_x = dx + rel.right + 1`;
  - `near_y = dy + rel.bottom + 1`, `far_y = −(dy + rel.top) + 1`;
  - `rel` is the ink box placed with its anchor at `(0, 0)`, corrected for
    origin rounding: `rel = box0 + (off − round(off))` (see Measurement).
  - The `+ 1` covers the ≤ 0.5 px origin rounding at any real position.

  Why per-side: a label above a point needs nothing below it, and a symmetric
  extent would lift a bar chart's baseline off the axis (GUP-544). GUP-528's
  solver already uses separate near and far constraints, so per-side costs one
  more `f32` per candidate. A layer's symmetric **reach** (the largest `near` or
  `far` of any label, at least 0) feeds `over[i]`, which remains the solver's
  cap, the `asked` trigger and `span_min`. `Overhang` and `Layer::overhang` in
  `selection.rs` are not changed.

- **Cap**: on each axis the text reach counts at most `0.25 ×` the first
  layout's plot width (x) or height (y):
  `over[i] = max(mark_over[i], min(text_reach[i], 0.25 × len_i))`. This keeps
  `span_min ≥ len/2 − pad > 0` (the property test's precondition). A label that
  reaches further is clipped, as before. Compute `lay = layout(counts)?` before
  `over` (the layout closure does not read `over`).

- **`Layout::mark_overhang`** stays the marks-only maximum, as documented. The
  visual-regression harness's confinement check reads it, and
  `multi_layer_png.rs` asserts 9.0.

- **Inset solver API** (`plot/inset.rs`):

  ```rust
  /// An edge candidate: normalized position, then the px a mark or label
  /// reaches towards the near end (left, bottom) and the far end.
  pub(crate) type Edge = (f32, f32, f32);
  pub(crate) fn edge_insets(cands: &[Edge], len: f32, pad: f32, cap: f32) -> (f32, f32);
  impl EdgeAxis {
      pub(crate) fn push(&self, out: &mut Vec<(f32, f32)>, t: f32, h: f32); // unchanged
      /// `push` for a one-sided candidate: binds if `near - t*span_min > 0`
      /// or `far - (1-t)*span_min > 0`; NaN extents are dropped.
      pub(crate) fn push_edge(&self, out: &mut Vec<Edge>, t: f32, near: f32, far: f32);
  }
  ```

  `Layer::edge_marks` keeps `[Vec<(f32, f32)>; 3]`, so `selection.rs` is not
  edited. `plot.rs` maps each mark candidate `(t, h)` to `(t, h, h)` and appends
  the text candidates before calling `edge_insets`.

- **Widen API** (`scale/mod.rs`, crate-private trait):
  `fn widen(&self, a: f32, b: f32, near: f32, far: f32) -> bool`. It sets the
  range to `(a + sign·near, b − sign·far)` and returns `false` when
  `near <= 0 && far <= 0` or the scale is not automatic. Callers pass
  `padded(near)` and `padded(far)` separately. GUP-529's rule (overhang +
  `plot_padding` when overhang > 0) now holds per end.

- **Automatic axes (x and left y only)**: before the `widened` array, for each
  numeric automatic axis (`is_auto()`, not `reads_keys()`) with
  `text_reach[i] > 0`:
  1. set the scale's range to the plot rect's ends (`left, right` or
     `bottom, top`);
  2. collect the text candidates with
     `EdgeAxis { range: those ends, span_min: len − 2·over[i] − 2·pad }`;
  3. solve `(tn, tf) = edge_insets(&cands, len, 2.0 * pad, over[i])`.

  Then call
  `widen(a, b, padded(mark_over[i].max(tn)), padded(mark_over[i].max(tf)))`.
  Without text candidates `tn = tf = 0`, so this equals today's call. `pad` is
  doubled because widening pads both ends. Solving with the smaller span is
  conservative.

- **Explicit axes**: in the existing `solve` block, after
  `layer.edge_marks(..)`, each text layer pushes its candidates for the asked x
  and left-y axes. `asked` and `cap` use the new `over[i]`. Moved views keep
  their stored insets (unchanged).

- **What a candidate is**: a label counts on an axis when its position there is
  a value (`Coords::Values`, not a key), its text is non-empty with ink, its
  pixel is finite, and `0 ≤ t ≤ 1` on that axis (`EdgeAxis::t` of the data
  position's pixel, without `dx`/`dy`, which are in the extents). The other
  coordinate is not checked, as with marks. Labels a `within` span would hide
  still count (a span depends on the ranges). This is conservative.

- **Band and point axes**: unchanged. They make their own room, `over` already
  drops non-size overhang there, and `asked` excludes them. Key positions never
  give candidates.

- **Right y axis**: no text candidates or reach. Text layers map y through the
  left axis only (GUP-526 owns the right axis).

- **Measurement** (`text_layer.rs`): add to the cached `Evaluated` a
  `measured: Option<Measured>`, where
  `struct Measured { size: f32, boxes: Vec<Option<(Bounds, [f32; 2])>> }` holds
  one entry per row. The tuple is the ink box `box0` of the run at `at = (0, 0)`
  (`text.ink_bounds`) and
  `off = text.font_weight(weight).baseline_origin(run at (0, 0))`. `None` marks
  an empty label or one with no ink. Measure when `measured` is `None` or its
  `size` differs from the effective size. At a position `at`, the ink box is
  `box0` shifted by `round(at + off) − round(off)` per component (exact: `walk`
  rounds the origin to whole pixels and places glyphs on integers from it).

- **Trait additions** (`AnnotationLayer`):

  ```rust
  /// Evaluate and measure (cached), then the largest px any label reaches
  /// past its position along [x, y], at least 0; [0, 0] for a layer
  /// missing x, y or text.
  fn reach(&mut self, text: &TextSystem, theme: &Theme) -> [f32; 2];
  /// Push a `(t, near, far)` candidate per label onto `out[0]` (x) and
  /// `out[1]` (y) for each `Some` axis, mapping positions through `x`
  /// and `y` at their current (provisional) ranges. Uses the cache
  /// `reach` filled; pushes nothing if it is empty.
  fn edges(
      &self,
      x: &dyn DynPositionScale,
      y: &dyn DynPositionScale,
      axes: [Option<&EdgeAxis>; 2],
      out: &mut [Vec<Edge>; 2],
  );
  ```

  `runs` calls the same "evaluate if needed" helper. A key on a numeric axis or
  a value on a key axis is skipped silently in `reach` and `edges`. `runs` still
  reports it as an error. `Derived` offsets are read where used, so a full
  resolve reads them more than once. The extents use the value read in step 5. A
  `dx_with` that depends on the final ranges is a band-slot offset, and band
  axes give no candidates.

- **Locking**: call `reach` with `cx.text()` held in its own block, dropped
  before `layout(..)` runs (that closure takes `cx.text()` itself), as step 3
  does.

- **Hidden labels**: GUP-542's rule is unchanged. A label is skipped if its
  anchor is more than 4 × size px outside the plot rect, and dropped if its ink
  misses the plot rect. Its bounds are clipped to the plot rect. After this
  story the rule still applies to labels outside the domain, labels after a zoom
  or pan, and labels beyond the cap.

- **Docs**: replace the `Plot::text` paragraph "Labels annotate data … cut off."
  with: labels never fit a domain, but the plot makes room for them. An
  automatic axis widens and an explicit one is inset, so a label whose point
  lies in the domain is drawn whole, using up to a quarter of the plot's width
  (height) per side. Labels are drawn above the data layers and below the axes,
  and are clipped to the plot rect: a label whose point is outside the domain
  (or moved there by a zoom or pan) or that reaches past the cap is cut off.
  Mirror this in one sentence in the `text_layer.rs` module doc.

- **Expected golden changes, with a cap**: exactly one stored golden changes,
  `tests/golden/gup_core/text_annotations.png`. The x range shrinks by about
  Sydney's half width, so the discs, the line and the labels move left. Every
  other golden, the dogfood images and the tutorials must be byte-identical.
  None of them has a text layer.

## Escalation Triggers

- Any golden other than `gup_core/text_annotations` changes, or any dogfood or
  tutorial test fails.
- A label that AC2 or AC3 expects whole is clipped by 1–2 px after the widened
  relayout (the plot rect moved after the auto solve). Report the sizes and the
  overflow. Do not add slack.
- The AC6 median delta is above 0.8 ms at low load, or above 1.1 ms at all.
- Making room needs `selection.rs`, `Overhang` or `Layer` to change.
- An existing GUP-528/529 test (`edge_inset*`, the tutorial-2 overhang test,
  `multi_layer_png`) changes its numbers.

## Out of Scope

- Labels on the right y axis (GUP-526), halos and overlap culling (GUP-527).
- Reserving margin for labels outside the plot rect (direct labels in a margin).
- Per-side widening for mark overhang. Marks stay symmetric. GUP-544's bar value
  labels use this story and need no headroom hook of their own.
- Dropping labels whose `within` span hides them from the extents.

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] `inset.rs`: `Edge`, `push_edge`, per-side `edge_insets`, tests (AC2 unit
      part); commit.
- [x] `text_layer.rs`: the `Measured` cache, the shared "evaluate if needed"
      helper, `reach`, `edges`, translated boxes in `runs` (or not, per AC1's
      rule); the exactness unit test; commit.
- [x] `scale/mod.rs`: `widen(a, b, near, far)`.
- [x] `plot.rs` step 5: layout before `over`, text reach and cap, marks-only
      `mark_overhang`, the automatic-axis solve and per-end widen, and text
      candidates in the explicit solve. Commit.
- [x] Integration tests for AC2–AC4 in `crates/gup-core/tests/text_layer.rs`.
- [x] Golden: update the assertions, bless, read the PNG; docs and doctest
      (AC5); commit.
- [x] Cost runs (AC6); `mask merge-check --dogfood`; retrospective.

## Dependencies

### Prerequisite Stories

- GUP-542: Text layer for annotations ✅: `TextLayer`, `AnnotationLayer`,
  `runs`.
- GUP-528: Data-aware edge insets ✅: `inset.rs`, the `solve` block.
- GUP-529: Shared-scale overhang clipping ✅: `padded`, the widen rule.

Runs alone among `plot.rs` stories. It must not run beside GUP-541 (tick
formats) or GUP-552 (subtitle), which also edit `plot.rs`.

### Enables Stories

- GUP-544: Bar value labels: room above (beside) the longest bar comes from this
  story, so 544 drops its headroom hook.

## Testing Strategy

- **Unit tests**: `edge_insets` with one-sided candidates and the property test;
  exactness of the translated ink boxes.
- **Integration tests**: explicit per-side insets, automatic per-side widening
  with an unchanged lower end, out-of-domain rows, band axes, the cap, the
  cached-resolve counter.
- **Visual validation**: the re-blessed `text_annotations` golden, read by eye.
- **Performance**: the release cost test, three runs.

## Success Metrics

- [x] Sydney is whole in `text_annotations.png`.
- [x] Only that one golden changed.
- [x] Cost delta ≤ 0.8 ms per 1,000 labels.

## Risk Assessment

- **Medium**: the automatic solve runs on the first layout's plot rect, and
  widening can change the y tick labels' width, and so the plot rect.
  _Mitigation_: the solve uses `2 × pad` and the extents carry 1 px of slack.
  The escalation trigger catches a residual clip.
- **Low**: the exactness of translated boxes rests on `walk`'s whole-pixel
  origin. _Mitigation_: AC1's test and its decision rule.
- **Low**: labels near (not at) an edge on a small plot may now claim room where
  they used to clip. This is intended, and capped at a quarter of the axis.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (`text_annotations.png`)
- [x] `mask wasm-browser` (via `mask merge-check --dogfood`): step 5 layout code
      changed, though no browser scene has a text layer
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

Text layers now report per-side extents, so labels at a point in the domain are
drawn whole.

- `plot/inset.rs`: `Edge = (t, near, far)`, `EdgeAxis::push_edge`, per-side
  `edge_insets` (new unit cases and the independent-sides property test).
- `text_layer.rs`: `Measured` cache (ink box at the origin plus baseline origin,
  per row, keyed on the effective size), `prepare`, `reach`, `edges` on
  `AnnotationLayer`; `runs` translates the cached box with `shift` and calls no
  `ink_bounds`. Unit test `a_translated_ink_box_equals_the_laid_out_one` (2
  weights x 9 anchors x 3 sizes x 3 texts x 50 positions, `==` on all four
  fields) passes, so the decision rule's first branch applies.
- `scale/mod.rs`: `widen(a, b, near, far)`.
- `plot.rs` step 5: layout before `over`; `mark_over` (marks only, feeds
  `mark_overhang` and the initial insets) and
  `over = max(mark_over, min(reach, quarter axis))`; automatic axes solve label
  room on the first plot rect with `2 * pad` and widen per end; explicit axes
  add label candidates to the solve.
- Docs: `Plot::text` rustdoc and doctest (Sydney at lon 150.0, centre within 1.5
  px of `run.at.x`), `text_layer.rs` module doc.
- Tests: 5 new integration tests in `crates/gup-core/tests/text_layer.rs` (AC2,
  AC3 x2, AC4 x2), the golden test now asserts Sydney whole. Only
  `tests/golden/gup_core/text_annotations.png` changed (re-blessed, read by eye:
  all six labels whole, Sydney ends inside the plot, threshold label as before).
- Cost: `resolve_cost_of_a_thousand_labels` release, loads 3.3/3.3/2.8: delta
  0.208, 0.205, 0.182 ms; median 0.205 ms (limit 0.8).
- `mask merge-check --dogfood`: PASS (gup-core, wasm-size, wasm-browser,
  dogfood). `cargo test --doc -p gup`: 51 + 4 passed. wasm scatter gz 340,915 B
  (budget 344,733).
- `scripts/perf_budget.sh --skip-wasm`: `zoom.cpu.median_ms` (0.848-0.880 vs
  0.828 limit), `zoom.cpu.p95_ms` (1.031-1.076 vs 1.075) and
  `zoom_culled.cpu.median_ms` (1.998-2.100 vs 1.488) fail, but the same three
  fail with `crates/` at the Start commit (0.848, p95 1.031, 2.057): host noise
  (GPU clock 350 MHz at idle), not this story. Not re-recorded.

## Retrospective

**Completed**: 2026-10-26

### Key Technical Learnings

#### Translating a measured ink box is exact

- **Challenge**: Avoid laying out every label at every full resolve.
- **Solution**: `walk` rounds the origin to whole pixels and places glyphs on
  integers from it, so the ink box at `at` is the box at `(0, 0)` shifted by
  `round(at + off) - round(off)`. The AC1 test found no difference at all (the
  largest difference is 0), so `runs` uses it and calls no `ink_bounds`.
- **Pattern**: When a layout snaps to the pixel grid, cache it at the origin and
  translate by whole pixels.

#### The doctest assertion fails without the feature

- Checked once by forcing the reach to zero: the doctest panicked with Sydney's
  bounds `x 439..468` around `at.x 460.5` (centre 453.5, 7 px off).

### Architectural Decisions

#### Initial insets use marks only

- **Decision**: The first provisional ranges (`insets = mark_over.map(..)`) use
  the marks' overhang, not the capped label reach; the solver then sets the
  explicit axis's insets from candidates.
- **Reasoning**: Label reach is a cap, not a need; insetting by it for the
  provisional range would skew every label's `t`. Only a moved view with no
  stored insets falls back to it, and then it gets the marks' value as before.
- **Trade-off**: None observed; all GUP-528/529 tests pass unchanged.

#### `reach` is zero along key-placed axes

- `reach` has no scales, so it takes the layer's own `Position::is_key`; the
  plot also zeroes a reach on any axis whose scale reads keys.

### Development Workflow Insights

- The worktree sandbox refused several multi-line shell commands (heredocs mixed
  with other commands); writing scripts with the Write tool and running
  `python3 -I` on them worked.
- Everything passed on the first run after wiring; the integration tests were
  written against the Decisions' formulas and none needed tuning.

### Follow-up Stories

None. GUP-556 and GUP-557 are unused.
