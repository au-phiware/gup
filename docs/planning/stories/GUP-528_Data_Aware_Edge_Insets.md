---
implementer: sonnet
implementer-reason:
  the rule, the data sources, the budget, the zoom policy and the signatures are
  fixed in Decisions; the work follows Layer::overhang and visible_entries
touches: [crates/gup-core/src/plot.rs, crates/gup-core/src/selection.rs]
---

# GUP-528: Data-Aware Edge Insets for Explicit Domains

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-25

## Context

GUP-540 insets an explicit-domain axis at its near end (left, bottom) by the
axis's overhang `over[i]`, and keeps the far end (right, top) at
`over[i] + Theme::plot_padding`. `over[i]` is the largest overhang of any layer
(a layer's maximum radius, half a constant bar width), so every explicit axis
loses that many pixels at both ends whether or not a mark sits near them. A
point at `(0, 0)` is now whole, but a chart whose marks keep away from the edges
pays the full inset anyway.

`tests/golden/gup_core/bubble_stroke.png` shows the worst case: the largest
radius is 60 px, so both axes lose 60 px at the near end and 64 px at the far
end, leaving a wide empty band at the bottom and left. Its bubbles are placed by
a pixel-identity scale (`pixels().encode(..)` in
`stroke_png::bubbles_with_translucent_fills_and_outlines`), not by the plot's
axes, so no inset can move them: the inset only moves the ticks and axis lines
away from them. `categorical_nulls.png` (explicit x `0..60000`, 5 px points)
pays a few pixels at each x end for no mark. `msaa_1x.png` and `scatter.png` are
the cases that must not regress: msaa's points sit on the domain corners (the
inset is needed there), and `scatter.png` has automatic domains (widened, not
inset).

Automatic domains already widen by the overhang in data units (`widen`, GUP-435,
GUP-494) and stay as they are. This story makes the explicit-domain insets
data-aware: each end of each axis is inset by the overhang of the marks that
actually reach it, computed from the layer's column data (or, where the CPU copy
is not kept, from the chunk statistics that culling already uses), never more
than GUP-540's inset.

## Read First

- `crates/gup-core/src/plot.rs` lines 1808–1880 (step 5: `over`, `inset`,
  `set_ranges`, `kept`, `widened`), 1912–1922 (relayout calls `set_ranges`
  again) and 1984–2010 (axis line extents read `inset`); lines 700–800 (`zoom`,
  `pan_from`, `record_home`, `reset_view`: `self.home` is `Some` while the view
  is moved).
- `crates/gup-core/src/selection.rs` lines 1137–1147 (`y_axis_id`: how a column
  encoding's scale identity is read), 1174–1215 (`Layer::overhang`), 1251–1327
  (`visible_entries`: chunk stats to pixels, the `ignored` ends), 1594–1600 and
  1660–1700 (`hit_items`: `retain.keeps_columns`, `cpu_values_where`,
  `cpu_eval`).
- `crates/gup-core/src/column.rs` lines 811–830 (`decode_chunk`) and 1183–1215
  (`cpu_values_where`, the pattern for decoding one chunk).
- GUP-540's Implementation Summary
  (`docs/planning/stories/GUP-540_Explicit_Domain_Edge_Inset.md`): the 8 goldens
  and 3 hand-computed tests it moved.

## User Story

> "As a visualization developer, I want an explicit domain to leave room only
> where a mark needs it, so that my chart is not padded by a large empty band at
> an edge no mark comes near."

## Acceptance Criteria

### AC1: The inset rule, as a pure function

- [ ] `edge_insets` (Decisions) has unit tests in `plot/inset.rs`: no candidates
      gives `(0.0, 0.0)`; `[(0.0, 5.0)]` gives `(5.0, 0.0)`; `[(1.0, 5.0)]`
      gives `(0.0, 5.0)`; `[(0.5, 5.0)]` with `len` 100 gives `(0.0, 0.0)`; and
      a property test over 1,000 candidate sets from a fixed-seed LCG written in
      the test (no new dependency) (`t` in `[0, 1]`, `h` in `[0, cap]`, `len` in
      `[50, 1000]`) checks, with `s = len - near - far - pad`, that every
      candidate satisfies `near + t * s >= h - 1e-3` and
      `far + (1 - t) * s >= h - 1e-3`, and that `near <= cap` and `far <= cap`.

### AC2: Each edge is inset by the marks that reach it

- [ ] New test `scatter_png::explicit_domain_insets_follow_the_edge_marks`: x
      and y domains `0..10`, 480×320, a `Circle` layer on the plot's axes with a
      radius column (pixel identity scale) and rows `(0.5, 5, r 30)`,
      `(9.9, 5, r 20)`, `(5, 9.8, r 15)`, `(5, 4, r 10)`. After resolve, with
      centres from `ScaleRef::eval`: the left disc's left edge lies within
      `[0, 1.5]` px inside `plot.left()`; the right disc's right edge and the
      top disc's top edge lie within `[pad, pad + 1.5]` px inside `plot.right()`
      and `plot.top()` (`pad` = `Theme::plot_padding`, 4 px: padding stays
      far-only); y value 0 maps to `plot.bottom()` (±0.5) (no mark reaches the
      bottom); every disc's colour is present 4 px inside its edge in the four
      directions (GUP-540's sampling).
- [ ] `scatter_png::points_on_an_explicit_domain_edge_are_whole` and
      `rect_png::the_first_bar_on_an_explicit_x_domain_is_whole_and_on_the_axis`
      (GUP-540) pass unchanged: a mark on the domain end still gets its full
      overhang.

### AC3: Axes no mark is bound to are not inset

- [ ] `stroke_png::bubbles_with_translucent_fills_and_outlines`: its bubbles are
      not encoded by the plot's scales, so both axes get near inset 0 and far
      inset `plot_padding`. Its assertions change from "first tick 60 px in" to:
      the first x tick within 1 px of the corner, the first y tick within 1 px
      of the corner, and the last ticks within 1 px of `plot.right() - 4` and
      `plot.top() + 4`. The comment names GUP-528.

### AC4: Released CPU data falls back to chunk statistics

- [ ] The AC2 scene with `.retain(Retain::GpuOnly)` renders with every disc
      whole (same sampling), and each of its four insets (near:
      `eval(0) − plot.left()`, `plot.bottom() − eval(0)`; far:
      `plot.right() − eval(10)`, `eval(10) − plot.top()`) is at least the AC2
      (`Retain::Auto`) inset and at most GUP-540's (`over`, `over + pad`).
- [ ] A 70,000-row layer (one chunk under default chunking, above
      `EDGE_ROW_BUDGET`), with a radius-20 point at the domain's lower-left
      corner, under `Retain::Auto` gives exactly the same `Layout::x_ticks` and
      `y_ticks` pixel positions as the same layer under `Retain::GpuOnly` (both
      took the statistics path).

### AC5: A moved view keeps its insets

- [ ] New test in `crates/gup-core/tests/events.rs` (or `zoom_uploads.rs`):
      explicit domains, a radius-20 point at `(0, 5)`; resolve;
      `plot.pan(30.0,     0.0)`; resolve: the pixel of x value 5 moved by 30.0 ±
      0.01. Then `reset_view()` and resolve: every tick's pixel equals the first
      resolve's (±0.01).
- [ ] `zoom_uploads.rs`, `resolve_cache.rs` and `events.rs` pass unchanged
      (zooming still writes no column bytes).

### AC6: Bars and lines are unchanged

- [ ] `gup_core/builder_bars`, `gup_core/grouped_bars`, `gup_core/rect_bars`,
      `gup_core/line_series`, `gup_core/builder_line`, `gup_core/scatter`,
      `gup_core/msaa_1x` and `gup_core/msaa_4x` pass without re-blessing, and
      `fixtures/msaa_guides.svg` is unchanged.

### AC7: Every moved golden is listed and read

- [ ] Re-blessed goldens (`GUP_BLESS=1`) are a subset of `bubble_stroke`,
      `categorical_nulls`, `wgsl_function_highlight`, `band_months`,
      `tick_format_currency` and `tick_format_percent`, each listed with what
      moved. The worker reads each PNG: `bubble_stroke`'s axes meet at the
      corner with the first ticks on it and no empty band; in the others every
      edge mark is whole and no gap is wider than the mark that needs it.
- [ ] Tests that compute pixels from the old inset by hand and break are fixed
      to read `Layout` or `ScaleRef::eval`, and listed.

### AC8: Every host agrees

- [ ] `mask wasm-browser` passes; scene code under `crates/gup-core/wasm-size/`
      that computes expected pixels by hand is updated and listed (none
      expected).
- [ ] `mask egui-screenshot` passes, and the worker reads its PNG: the egui
      scatter's `(0, 0)` point is still a whole disc.
- [ ] `./dogfood/run_all.sh` reports every scenario PASS.

## Decisions

- **The rule.** For a non-widened, non-ordinal axis of length `len` px (plot
  width for x, height for y and right y) whose marks are candidates `(t, h)`
  (`t` the mark's position normalized to the axis range, 0 at the near end, 1 at
  the far end; `h` its half-extent in px along the axis), the near inset `n` and
  far inset `f` satisfy `n + t·s >= h` and `f + (1 − t)·s >= h` for every
  candidate, where `s = len − n − f − pad` is the final span. The far end is
  then inset by `f + pad`, the near end by `n` (padding stays far-only, as in
  GUP-540). Rows whose `t` is outside `[0, 1]` (outside the domain) are not
  candidates: they are clipped by design.
- **The solver** (new, `crates/gup-core/src/plot/inset.rs`, declared
  `pub(crate) mod inset;` in `plot.rs`, no `lib.rs` change):

  ```rust
  /// Near and far insets (px, the far one without padding) of an axis
  /// `len` px long whose marks are `cands` as `(t, h)`; each at most `cap`.
  pub(crate) fn edge_insets(cands: &[(f32, f32)], len: f32, pad: f32, cap: f32) -> (f32, f32)
  ```

  Start from `s = len − 2·cap − pad` (GUP-540's span, the smallest possible).
  Three times: `n = max(0, max h − t·s)`, `f = max(0, max h − (1 − t)·s)`, both
  capped at `cap`, then `s = len − n − f − pad`. Return the last `(n, f)`. Each
  pass only grows `s`, so every pass's insets satisfy the rule for the span that
  follows (no mark is clipped), and three passes leave under 0.5 px of slack in
  practice. `cap` is `over[i]`, so no inset exceeds GUP-540's.

- **Candidates come from the layers**, through a new `Layer` method in
  `selection.rs`:

  ```rust
  /// GUP-528: the marks that can reach within an axis's edge allowance,
  /// pushed onto `out[i]` as `(t, h)` for each asked axis `axes[i]`.
  fn edge_marks(&self, axes: &[Option<EdgeAxis>; 3], budget: &mut usize, out: &mut [Vec<(f32, f32)>; 3]);
  ```

  with
  `pub(crate) struct EdgeAxis { identity: usize, range: (f32, f32), span_min: f32 }`
  in `plot/inset.rs` (`range` is the provisional pixel range, `span_min` the `s`
  the solver starts from). A candidate is pushed only if `h − t·span_min > 0` or
  `h − (1 − t)·span_min > 0`; others can never bind.

- **Which channels count**: X-role column channels for axis 0, Y-role column
  channels for axes 1 and 2, skipping the ends `visible_entries` ignores
  (`x_end`/`y_end` of a centre-plus-extent box; factor that closure into a
  helper both use), and only when the channel's scale identity, read as
  `y_axis_id` reads it (`c.links().first()?.dictionary_slot()` as a pointer),
  equals `EdgeAxis::identity`. Constant and derived positions and positions
  encoded by any other scale do not count: the range does not move them, so an
  inset cannot help them (this is why `bubble_stroke` gets no inset).
- **Half-extent `h`** per row on axis x: `max(size_row, const_x)`, where
  `size_row` is `|cpu_eval(v)|` of the Size-role column for that row (or the
  constant size, as `Layer::overhang` reads it) and `const_x` is half a constant
  or derived Width (`Overhang::x`); on y the same with Height. Column-encoded
  widths and heights are not counted, as in `Layer::overhang`.
- **Per row or per chunk.** If the layer's policy keeps its CPU columns
  (`self.retain.keeps_columns(store.rows())`) and the chunk's rows fit in the
  remaining `budget`, decode that chunk's position (and Size) column with a new
  `ColumnStore::chunk_values(&self, k: usize, column: usize) -> Option<Vec<f64>>`
  in `column.rs` (one-chunk `cpu_values_where`), push per-row candidates (NaN
  rows skipped) and subtract the chunk's rows from `budget`. Otherwise push the
  chunk's statistics bound: `t_lo`, `t_hi` from the image of the chunk's
  position stats (as `visible_entries` computes it), clamped to `[0, 1]`, with
  `h_max` from the chunk's Size stats, as two candidates `(t_lo, h_max)` and
  `(t_hi, h_max)`; skip a chunk entirely outside `[0, 1]`. The bound is never
  tighter than the exact rows and never looser than GUP-540. Deciding by policy,
  not by whether the copy is still there, keeps the first resolve (before
  `release`) and later ones identical.
- **Budget**: `pub(crate) const EDGE_ROW_BUDGET: usize = 65_536` rows decoded
  per resolve, shared by all layers and axes, chunks taken in store order. Below
  it, edge insets are exact; above it, the statistics path costs O(chunks).
- **When it runs**: in step 5, after `widened` is known, call `set_ranges` with
  GUP-540's insets (the provisional range), collect candidates for each axis
  that is not widened, not `reads_keys()`, has `over[i] > 0` and is not frozen,
  then call `set_ranges` with the solved insets. After a relayout (the
  `again.plot != lay.plot` branch) solve again from the same candidates with the
  new `len` (`t` does not depend on the range). `set_ranges` takes
  `[(f32, f32); 3]` (near, far-without-pad) instead of reading `over`.
- **Axis lines** read the same far inset: `plot.right() − (f + pad)`,
  `plot.top() + (f + pad)`, for the right y axis likewise (replacing `inset[i]`
  in the axis-line block).
- **Ordinal axes keep GUP-540's rule** (near `over`, far `over + pad`): bands
  make their own room and per-row band positions are out of scope. Widened axes
  are unchanged (range = plot rect). An axis with `over[i] == 0` gets `(0, 0)`,
  which is today's result.
- **A moved view freezes the insets.** A new field
  `edge_insets: Option<[(f32, f32); 3]>` on `Plot` stores the solved insets of
  each resolve made while `self.home.is_none()`. While `self.home.is_some()`
  (after a zoom or pan, until `reset_view`), explicit axes use the stored insets
  (or GUP-540's if none were stored), so a pan moves content by exactly the
  dragged pixels. `reset_view` needs no change: `home` becomes `None` and the
  next resolve solves again.
- **No option and no new public API.** The new insets are never larger than
  GUP-540's and never clip a mark bound to the axis; a user who wants a gap
  widens the domain. `Layout::mark_overhang` keeps its meaning (the largest
  overhang).

## Escalation Triggers

- A position channel encoded with the plot's own `ScaleRef` (as in the AC2 test)
  does not report the axis's identity through
  `links().first()?.dictionary_slot()`, so it is not a candidate: stop; the
  identity rule needs a different source.
- Any AC6 golden or fixture changes, or a golden outside the AC7 list changes:
  stop and list it with what moved.
- The AC1 property test finds a candidate the solver leaves clipped: stop; do
  not add passes or margins.
- A zoom or cache test fails in a way that is not a pixel position (a column
  write count, a cache miss count): stop and report.
- `edge_marks` needs `&mut self` (for example to evaluate rows through
  accessors): stop; the rows path is deliberately not used.

## Out of Scope

- Automatic-domain widening (GUP-435, GUP-494): unchanged, including its use of
  the maximum overhang at both ends.
- Data-aware insets on band and point axes.
- Evaluating rows through accessors under `Retain::Rows` (statistics are used).
- Unclipped marks and a user-facing inset option.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [ ] `plot/inset.rs`: `EdgeAxis`, `EDGE_ROW_BUDGET`, `edge_insets` and the AC1
      tests; commit.
- [ ] `column.rs`: `ColumnStore::chunk_values`. `selection.rs`: the scale
      identity helper (shared with `y_axis_id`), the ignored-end helper (shared
      with `visible_entries`) and `Layer::edge_marks`.
- [ ] `plot.rs` step 5: provisional ranges, candidates, solved ranges, relayout
      re-solve, axis lines, the `edge_insets` freeze field; run gup-core tests
      with `--no-fail-fast` and list failures; commit.
- [ ] AC2–AC5 tests; fix hand-computed tests; re-bless and read goldens in a
      commit of their own.
- [ ] One sentence in the rustdoc of `Selection::retain`: policies that drop the
      CPU columns inset explicit-domain edges from chunk statistics.
- [ ] `mask wasm-browser`, `mask egui-screenshot`, dogfood, `mask wasm-size`.

## Dependencies

### Prerequisite Stories

- GUP-540: Marks on an Explicit Domain's Near Edge Stay Inside the Plot ✅ — the
  near-end inset and `near(i)` this story replaces with data-aware insets.
- GUP-494 ✅ — per-axis overhang (`Overhang::x`, `y`, `size`).

### Enables Stories

- GUP-548 (histogram) and GUP-547 (stacked areas): their explicit-domain ends
  get the tighter rule without further work.

## Testing Strategy

- **Unit tests**: `edge_insets` cases and the property test (AC1).
- **Integration tests**: AC2–AC5 pixel and layout tests; the two GUP-540 tests;
  every gup-core test.
- **Visual validation**: every re-blessed golden read by eye; the egui
  screenshot.
- **Performance**: the budget bounds decoding at 65,536 rows per resolve; the
  70,000-row test (AC4) proves the statistics path is taken above it.

## Success Metrics

- [ ] No explicit-domain axis is inset at an end that no axis-bound mark
      reaches, and no axis-bound mark inside the domain is clipped.
- [ ] At most the 6 goldens in AC7 change.

## Risk Assessment

- **Medium**: candidates read the column encodings' `cpu_eval` under the
  provisional range; a scale whose CPU mirror is missing returns `None` and its
  rows are skipped, which could clip a mark. _Mitigation_: a `None` from
  `cpu_eval` or `image` on a chunk makes that chunk use `(0, h_max)` and
  `(1, h_max)`, i.e. GUP-540's inset.
- **Low**: the inset now depends on `Retain` (exact versus statistics), so two
  plots of the same data with different policies may differ by a few pixels.
  _Mitigation_: documented in the rustdoc of `Selection::retain`; both are
  clip-free.
- **Low**: freezing insets while the view is moved means a mark brought to the
  edge by a zoom may be cut, as before GUP-540. That is what a zoomed view does
  at its edges.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (re-blessed goldens, egui screenshot)
- [ ] `mask wasm-browser` passes (layout change)
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
