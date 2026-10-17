---
implementer: sonnet
implementer-reason:
  hit geometry, value sources, draw-order mapping and the grid are all specified
  in Decisions; follows the existing column-store and DrawOrder code
---

# GUP-445: RFC-001 Step S8b: CPU Picking

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-17

## Context

RFC-001 §9 specifies `chart.pick(px) -> Option<&T>` and
`pick_row(px) -> Option<RowId>`, evaluated on the CPU over the retained columns
into a lazily built uniform grid in pixels. The GPU path (`pick_async`) is S9.
Today nothing in `gup-core` can say which row is under a point. The old path's
`InteractionData` hit-tests positions the user supplies, not the positions that
were drawn.

Three things make "the row that was drawn here" non-trivial, and the RFC
findings record all three:

- **Chunks and origins.** Columns are stored per chunk, relative to an f64
  origin (S4a). A CPU copy survives only under some `Retain` policies (S4b,
  `selection.rs` lines 27–90).
- **Draw order.** A layer with a size column stores its rows largest first.
  `DrawOrder::row_of` maps a stored row back to the data row (`order.rs` lines
  162–172, "GUP-431 findings → Picking"). Its `segments` give the order in which
  rows draw, so they say which mark is on top.
- **Lines.** A `Line` instance is the segment from a row to the next one, and a
  null row breaks the line ("GUP-432 findings → Proposed adjustments").

Each library mark has its own shape: `Circle` is a disc, `Rect` an aligned box
(centre and extent, or two edges), `Segment` a square-capped stroke, `Line` a
chain of capsules. The hit test needs each mark's shape, so this story gives
`Mark` a declared hit shape.

## Read First

- RFC-001 §9 "Picking" (lines 642–654).
- RFC-001 "S4b findings → Proposed adjustments" S9 bullet (lines 2316–2317),
  "GUP-431 findings → Mechanism" (lines 3064–3090) and "GUP-432 findings →
  Proposed adjustments" (lines 3294–3302).
- [GUP-444](GUP-444_RFC_001_S8a_Cached_Resolve_And_Input.md) "Decisions"
  (`resolve_generation`, `Chart`).
- `crates/gup-core/src/marks.rs` (the four marks' channels and semantics).
- `crates/gup-core/src/channel.rs` lines 284–331 (`Geometry`, `Mark`).
- `crates/gup-core-macros/src/mark.rs` lines 18–70 (attribute parsing) and
  515–580 (generated `Mark` impl).
- `crates/gup-core/src/selection.rs` lines 27–125 (`Retain`, `Selection`),
  279–340 (`column_encodings`, `evaluate`) and 509–540 (`Layer`).
- `crates/gup-core/src/order.rs` lines 27–45 and 150–172.
- `crates/gup-core/src/column.rs` lines 1–54 (layout and validity), 380–410
  (`ChunkColumn`) and 495–580 (`Chunk`).
- `crates/gup-core/src/encoding.rs` lines 955–1064 (`DynColumnEncoding`).
- `crates/gup-core/src/scale/mod.rs` lines 288–293 (`monotonic_image`).

## User Story

> "As a visualization developer, I want to ask a chart which of my rows is drawn
> under a point, and get my own `&T` back, so that I can build tooltips,
> selection and drill-down without recomputing positions myself."

## Acceptance Criteria

### AC1: Hit shapes on marks

- [ ] `HitShape` and `Mark::HIT` exist as in Decisions. `#[mark(hit = ..)]` sets
      it, and `Circle`, `Rect`, `Segment` and `Line` declare `Disc`,
      `AlignedBox`, `Segment` and `Polyline`.
- [ ] A new trybuild case `tests/derive_mark_fail/hit_missing_channel.rs`
      (`hit = Disc` on a mark with no `radius`) fails with the message in
      Decisions. `hit = Polyline` on a `Quad` mark fails too, in the same file
      or a second one.

### AC2: Hits per mark kind

In a new `crates/gup-core/tests/pick.rs`, each plot is resolved at 480×320 with
explicit domains:

- [ ] **Circle** (three rows, radius 6): the centre, `r − 0.5` and `r + 2.5`
      from the centre pick the row; `r + 3.5` picks `None`. For every row, the
      pixel of the rendered image at `pick_anchor` (Decisions) has that row's
      fill (ΔE < 2), which shows that the CPU positions are the drawn ones.
- [ ] **Overlap**: two overlapping discs of constant radius pick the later row
      in the overlap. With `RADIUS` encoded from a column, the smaller disc
      (drawn last) wins. A point inside a lower disc and within tolerance of the
      topmost one's edge picks the lower disc.
- [ ] **Rect**: centre-and-extent bars (`width` from `band_width_px`, `Band` x)
      and edge bars (`y_end` a `Derived` baseline) each pick their row inside
      and `None` 4 px outside.
- [ ] **Segment** (horizontal, width 4, from pixel `A` to pixel `B`, both read
      from the scales): `B + (1.5, 0)` picks it (square cap), `B + (4.5, 0)`
      picks it (tolerance), and `B + (6, 0)` and the midpoint `+ (0, 5.5)` pick
      `None`.
- [ ] **Line** (rows 0–4, row 2 null, width 2): a point on segment 0–1 nearer
      row 1 picks row 1. Points where segments 1–2 and 2–3 would run pick
      `None`. A point near row 4 picks row 4.

### AC3: Rows across chunks, nulls, draw order and retention

- [ ] `max_chunk_rows(4)` on a 10-row circle layer gives the same picks as one
      chunk at all ten centres.
- [ ] A row with a NaN `y` is never picked.
- [ ] Unit test in `src/pick.rs`: a size-encoded layer, then `append` of a batch
      (sorted into the middle of the draw order). Every row's centre picks its
      own data row index, and in an overlap the row drawn later wins.
- [ ] `Retain::Rows` after a resolve (CPU columns released): `pick_row` and
      `pick::<T>` still work. `Retain::Columns`: `pick_row` works and
      `pick::<T>` returns `None`. `Retain::GpuOnly` with more rows than one
      chunk: `pick_row` is `None` for that layer, and a second layer still
      picks.
- [ ] Two layers: the higher `z` wins where they overlap, even when added first.
      `RowId.layer` is the index in `add` order.
- [ ] `None` before any resolve and outside `layout.plot`. After `handle(Wheel)`
      and a resolve, a row's new centre picks it.
- [ ] A user mark without `hit` (the `derive_mark` test's mark) is never picked.

### AC4: Performance

- [ ] `#[ignore]`d `pick_100k_points` in `tests/pick.rs`, run with
      `cargo test -p gup-core --release --test pick -- --ignored`: 100,000
      circles at 800×500. The first pick (index build) takes ≤ 50 ms, and the
      mean of 10,000 seeded random picks is ≤ 20 µs. The test asserts both and
      prints them, and the Retrospective records them.

### AC5: Public API from an external crate

- [ ] A doctest on `Plot::pick` builds a three-row scatter, resolves it, and
      asserts that `pick::<Row>` at the second row's position returns that row
      (`row.name == "b"`).

## Decisions

`crates/gup-core/src/channel.rs`:

```rust
/// How CPU picking hit-tests a mark's rows (RFC-001 §9).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum HitShape {
    /// Never picked on the CPU (the default for user marks).
    #[default]
    Unpickable,
    /// A disc at (`x`, `y`) of radius `radius`.
    Disc,
    /// An axis-aligned box: per axis, centre and extent (`width`, `height`)
    /// when the extent is non-zero, else the edges `x`..`x_end` (`y`..`y_end`).
    AlignedBox,
    /// `(x, y)`–`(x_end, y_end)`, `width` wide, square caps.
    Segment,
    /// Capsules from each row to the next, `width` wide (`Geometry::Polyline`).
    Polyline,
}
// in trait Mark:
const HIT: HitShape = HitShape::Unpickable;
```

- **Derive**: `#[mark(.., hit = Disc)]`. Expansion checks that the mark has
  every channel the shape reads (names as above, all of type `Px`) and that
  `Polyline` goes with `geometry = Polyline` and the others with `Quad`.
  Message: "`hit = Disc` reads channels `x`, `y` and `radius`; this mark has no
  `radius`". `USAGE` mentions `hit`.
- New `crates/gup-core/src/pick.rs`:

```rust
/// A row of a plot layer: `layer` is the index in `Plot::add` order, `row` the
/// index into the rows the `Selection` was given (appended rows continue it).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RowId { pub layer: usize, pub row: usize }

/// How far outside a mark a point still picks it.
pub const PICK_TOLERANCE: Px = Px(3.0);

impl Plot {
    /// The topmost row drawn under `at` (logical px), or `None`.
    pub fn pick_row(&mut self, at: Point) -> Option<RowId>;
    /// The topmost row's data, if its layer holds rows of `T` and kept them.
    pub fn pick<T: 'static>(&mut self, at: Point) -> Option<&T>;
    /// Where a tooltip for `row` attaches (see "Anchors"); `None` if not drawn.
    #[doc(hidden)]
    pub fn pick_anchor(&mut self, row: RowId) -> Option<Point>;
}
// Chart gains: fn pick_row(&mut self, at: Point) -> Option<RowId>;
```

- **Signed distance `d`** (≤ 0 inside), with `h = width / 2`:
  - `Disc`: `|p − c| − r`.
  - `AlignedBox`: the signed distance to the box from the rules in `HitShape`.
  - `Segment`: the signed distance to the rectangle `u ∈ [−h, L + h]`,
    `v ∈ [−h, h]` in the segment's frame. When `L = 0`, an axis-aligned square
    of side `2h` centred on the point.
  - `Polyline`: the distance from `p` to segment `i → i+1`, minus `h`, where `h`
    comes from row `i`. Segment `i` exists only if rows `i` and `i+1` both have
    finite positions. It picks the endpoint row nearer to `p` (row `i` on a
    tie).
- **Hit** when `d ≤ PICK_TOLERANCE`. **Winner**: among hits with `d ≤ 0`, the
  topmost. If there are none, the smallest `d`, with ties going to the topmost.
  **Topmost** is the lexicographic maximum of (layer `z`, layer index, the row's
  draw rank).
- **Draw rank**: with a `DrawOrder`, the stored row's position when walking
  `segments()` in order (make `segments` `pub(crate)`, not test-only). Without
  one, the stored row (= data row). For a polyline, the segment index.
- **Anchors** (used by GUP-446): `Disc`, the centre. `AlignedBox`, the middle of
  its top edge (smallest y). `Segment`, the midpoint. `Polyline`, the picked
  row's point.
- **Values per channel**, with the first source that applies used for the whole
  layer:
  1. Every chunk keeps its CPU copy: new
     `ColumnStore::cpu_values(&self, column: usize) -> Option<Vec<f64>>` in
     stored order (origin + hi (+ lo), codes as `f64`; NaN for a cleared
     validity bit or `NULL_CODE`; `None` if any chunk was released). Map stored
     → data rows with `DrawOrder::row_of`; drop its `expect(dead_code)`.
  2. Else, if the rows are kept (`released == 0`), `DynColumnEncoding::evaluate`
     over the rows, in data order. Get the stored index of data row `i` by
     inverting `row_of` over all stored rows once per build.
  3. Else the layer is skipped (`GpuOnly` above one chunk; S9's GPU pick).
- **To pixels**: a column through new
  `DynColumnEncoding::cpu_eval(&self, v: f64) -> Option<f64>`, whose default is
  `self.image((v, v)).map(|(a, _)| a)` (every library scale's `image` is
  monotonic). `Const(F32(v))` gives `v`. `Derived(f)` gives `f()`, called once
  per build. An unset channel gives its `ChannelDesc::default`. `None` or a
  non-finite value makes the row unpickable.
- **Index**: `PickIndex` in `pick.rs`, a uniform grid of 32 × 32 px cells over
  `layout.plot`. Each item (row id, rank, shape, anchor) goes into every cell
  its bounding box, grown by `PICK_TOLERANCE`, overlaps within the plot rect.
  Build it lazily on the first pick after `resolve_generation()` changed or
  after `handle(Wheel)` zoomed. A point outside `layout.plot` picks `None`,
  because marks are clipped to it.
- **`Layer` additions**:
  `fn hit_items(&mut self, layer: usize, out: &mut Vec<PickItem>) -> Result<()>`
  (pushes nothing for an unpickable or skipped layer) and
  `fn row_any(&self, row: usize) -> Option<&dyn Any>`. `pick::<T>` downcasts the
  topmost hit's row and does not look at lower layers.
- A failed build (a `Result` from `evaluate`) makes `pick_row` return `None`.
  `pick_row` does not return a `Result`, matching `handle`.

## Escalation Triggers

- For any test row the CPU position differs from the drawn one by more than 0.5
  px (AC2's pixel check fails): the CPU mirrors disagree with the GPU.
- A library scale's `image((v, v))` is `None` for a finite value inside its
  domain, or `Band`/`Point` codes do not map to slot centres through it.
- Picking needs anything written to or read back from the GPU.
- `cpu_values` cannot reconstruct a hi/lo (`F32x2Relative`) value to within 1e-6
  relative.

## Out of Scope

- GPU picking (`pick_async`) and `RowId` under `GpuOnly`: RFC-001 S9.
- Making S9's GPU line pick agree with "nearer endpoint": S9 (noted in
  findings).
- Hover, click and tooltips: GUP-446.
- Hit shapes for user marks beyond `HitShape`, and areas: later.

## Technical Tasks

- [ ] `HitShape`, `Mark::HIT`, the derive attribute and its checks, and the
      trybuild case(s). Annotate the four library marks.
- [ ] `ColumnStore::cpu_values` with unit tests for each `ColumnFormat`,
      including nulls and a released chunk.
- [ ] `DynColumnEncoding::cpu_eval`.
- [ ] `pick.rs`: the distance functions (unit-tested), `PickItem`, `PickIndex`,
      and the `Layer` methods on `Selection`.
- [ ] `Plot::pick_row`, `pick`, `pick_anchor`, and `Chart::pick_row`.
- [ ] `tests/pick.rs` and the unit tests for append and draw order.
- [ ] Append "S8b findings (GUP-445)" to RFC-001: the sources per `Retain`, the
      timings, and the line-endpoint rule for S9.

## Dependencies

### Prerequisite Stories

- GUP-444: RFC-001 S8a cached `resolve` and `Chart::handle` 📋 —
  `resolve_generation`, `layout()`, the `Chart` shape.
- GUP-431: draw order ✅, GUP-432: `Line` ✅, GUP-415: `Retain` ✅.

### Enables Stories

- GUP-446 (S8c hover, click, tooltips) — `pick_row`, `pick_anchor`.

## Testing Strategy

- **Unit tests**: the distance functions, `cpu_values` per format, and append
  with draw order.
- **Integration tests**: `tests/pick.rs` (AC2, AC3, AC5).
- **Visual validation**: AC2's pixel check at each anchor on the rendered image.
- **Performance**: AC4, in release.

## Success Metrics

- [ ] Every row of every AC2/AC3 fixture picks itself at its anchor.
- [ ] AC4 is within its thresholds.

## Risk Assessment

- **Medium**: this story is broad (four shapes, three sources, draw order).
  _Mitigation_: each piece has its own unit test, and the distance functions are
  pure.
- **Low**: `image((v, v))` as an evaluator depends on `image` staying an
  end-point image. _Mitigation_: `cpu_eval` is one method, overridable later.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] `mask wasm-size` passes (re-record `wasm.scatter.gz_bytes` if it moved)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
