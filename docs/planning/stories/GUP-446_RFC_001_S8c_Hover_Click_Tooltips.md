---
implementer: sonnet
implementer-reason:
  event semantics, tooltip content, style and placement are fixed in Decisions;
  builds on S8a's handle and S8b's pick
---

# GUP-446: RFC-001 Step S8c: Hover, Click and Tooltips

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-17

## Context

RFC-001 §9 says `Plot` resolves the hit and dispatches it to handlers registered
with `sel.on(Event::Click, |ev, d: &T| …)`. It lists the tooltip as a built-in
behaviour, and §10(b) shows `.tooltip(|d: &Row| format!(..))`. The strategic
review (T5) lists "built-in tooltips and picking" among the features the dogfood
audit found missing.

After [GUP-444](GUP-444_RFC_001_S8a_Cached_Resolve_And_Input.md),
`Chart::handle` takes pointer input but only the wheel does anything. After
[GUP-445](GUP-445_RFC_001_S8b_CPU_Picking.md), `Plot::pick_row` finds the
topmost row under a point, and `pick_anchor` says where its mark is. This story
adds the state machine between them: what is hovered, what was clicked, which
handlers run, and a tooltip drawn on top of the cached scene, without
re-resolving the plot.

Interaction is not fully testable with goldens. Each behaviour gets a synthetic
`Input` sequence with asserted results, and the tooltip gets one golden.

## Read First

- RFC-001 §9 "Events" (lines 655–666) and §10(b) and (c) (lines 687–712).
- [GUP-444](GUP-444_RFC_001_S8a_Cached_Resolve_And_Input.md) and
  [GUP-445](GUP-445_RFC_001_S8b_CPU_Picking.md) "Decisions".
- `crates/gup-core/src/plot.rs` lines 36–67 (`Layout`, `Resolved`) and 610–660
  (how `resolve` pushes items and builds `Layout`).
- `crates/gup-core/src/scene.rs` lines 14–60 (z constants, `Scene::push`),
  195–210 (`RectPrim`) and 255–300 (`TextStyle`, `TextRole`, `TextRun`).
- `crates/gup-core/src/theme.rs` (all of it).
- `crates/gup-core/src/selection.rs` lines 100–160 (`Selection` fields, `new`).
- `crates/gup-core-macros/src/mark.rs` lines 23–32 (`SELECTION_METHODS`).
- `crates/gup-core/tests/common/vr.rs` lines 28–35 (`role`).

## User Story

> "As a visualization developer, I want to attach a tooltip and click handlers
> to a layer in one line each, so that readers can inspect individual data
> points without me writing hit-testing or drawing code."

## Acceptance Criteria

### AC1: Events from synthetic input

In a new `crates/gup-core/tests/events.rs` (three circles, rows 0–2, resolved at
480×320; anchors from `Plot::pick_anchor`):

- [ ] Moving 0 → 1 → (empty space) → `PointerLeft` calls handlers in exactly
      this order: `Enter(0)`, `Leave(0)`, `Enter(1)`, `Leave(1)`. Each
      `EventInfo` has the pointer position and `RowId`, and `Plot::hovered()`
      follows along.
- [ ] Moving within row 1's disc calls no handler.
- [ ] `PointerDown(Primary)` at row 2 and `PointerUp(Primary)` 3 px away calls
      `Click(2)` once. Moving 5 px between down and up calls none. Secondary and
      Middle buttons call none.
- [ ] A handler that pushes the clicked row's name into an
      `Arc<Mutex<Vec<String>>>` receives `&T` (the rows are a struct with a
      `name`).
- [ ] Under `Retain::Columns`, no handler runs (no `&T`), but `hovered()` still
      changes.
- [ ] A wheel zoom while row 0 is hovered calls `Leave(0)` and clears
      `hovered()`.

### AC2: Redraw only when the tooltip changes

- [ ] Over a layer with `.tooltip(..)`, entering, changing or leaving a row
      returns `redraw: true`. Moving within the same row, or over a layer
      without a tooltip, returns `redraw: false`.

### AC3: Tooltip overlay without re-resolving

- [ ] After hovering row 1, the next `resolve` leaves `resolve_generation()`
      unchanged and writes 0 bytes of `Column`, `Validity` and `Uniform`
      (`Texture` may grow with new glyphs). Its scene has one `Rects` item and
      one `Text` item at `Z_TOOLTIP`, with one `TextRun` per line of the tooltip
      text (role `TextRole::Tooltip`). `layout().tooltip` is `Some`.
      `layout().texts` holds no tooltip run.
- [ ] After `PointerLeft`, the next resolve's scene has no `Z_TOOLTIP` items and
      `layout().tooltip` is `None`.
- [ ] After a resize while hovering (a full resolve), the tooltip is re-anchored
      at the row's new anchor.

### AC4: Tooltip golden and placement

- [ ] Golden `tests/golden/gup_core/tooltip.png` (480×320): five rows
      `{ name, mass }` with
      `.tooltip(|r| format!("{}\nmass {:.1} kg", r.name,     r.mass))`, hovering
      the middle row. The implementer reads the PNG: a white box with a 1 px
      grey border sits 8 px above and right of the disc's centre, two
      left-aligned dark lines ("Kiwi", "mass 2.4 kg") sit inside it with 6 px
      padding, and no glyph is clipped.
- [ ] Placement test: hovering a row 5 px from the plot's top-right corner gives
      a `layout().tooltip` rect left of and below the anchor, inside
      `[0, 480] × [0, 320]`, with whole-pixel edges. A text longer than the
      canvas is clamped to `x = 1`.
- [ ] An empty tooltip string draws nothing (`layout().tooltip` is `None`).

### AC5: Names and defaults

- [ ] `SELECTION_METHODS` in `gup-core-macros` gains `on` and `tooltip`, and the
      already-missing `z` and `legend_title`. `derive_mark_fail` snapshots
      change only if one names those.
- [ ] `Theme::default()` gains the four tooltip fields in Decisions, and
      `theme.rs`'s `default_is_the_old_styling` test asserts them.
      `tests/common/vr.rs` maps `TextRole::Tooltip` to `VrTextRole::Other`.
- [ ] `mask wasm-browser` and `mask wasm-size` pass (re-record
      `wasm.scatter.gz_bytes` if it moved). `mask perf-budget` passes (a
      timing-only failure needs a second run).

## Decisions

In `crates/gup-core/src/input.rs`:

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Event { Enter, Leave, Click }

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EventInfo { pub event: Event, pub at: Point, pub row: RowId }
```

On `Selection<T, M>`:

```rust
/// Call `handler` with the row when `event` happens on one of this layer's
/// marks. Not called for rows dropped by `Retain`.
pub fn on(&mut self, event: Event,
          handler: impl FnMut(&EventInfo, &T) + Send + Sync + 'static) -> &mut Self;
/// Show `text(row)` beside a hovered row; '\n' separates lines.
pub fn tooltip(&mut self, text: impl Fn(&T) -> String + Send + Sync + 'static) -> &mut Self;
```

On `Plot`: `pub fn hovered(&self) -> Option<RowId>`.

- **State** in `Plot`: `hover: Option<(RowId, Point /* anchor */)>` and
  `press: Option<Point>` (the Primary button only).
- **`handle`**:
  - `PointerMoved { at }`: `pick_row(at)`. If it differs from the hovered row,
    dispatch `Leave` (old) and then `Enter` (new), and update `hover` with
    `pick_anchor`.
  - `PointerLeft`: `Leave` (if any), and clear `hover`.
  - `PointerDown { Primary }` sets `press`. `PointerUp { Primary }` dispatches
    `Click` on `pick_row(at)` if `|at − press| ≤ 4.0` (`CLICK_SLOP`, private),
    then clears `press`. Other buttons do nothing.
  - `Wheel` that zooms: `Leave` (if hovering), clear `hover`, then zoom
    (GUP-444).
  - `redraw` is `true` exactly when the tooltip shown changes: hovering a row
    whose layer has a tooltip, or leaving one.
- **Dispatch**: the new `Layer::dispatch(&mut self, info: &EventInfo)` runs the
  layer's handlers for `info.event` in registration order, with `&T` from
  `row_any`. Handlers are
  `Vec<(Event, Box<dyn FnMut(&EventInfo, &T) + Send + Sync>)>` on `Selection`.
  `Layer::tooltip_text(&self, row: usize) -> Option<String>` returns `None`
  without a tooltip, for a released row, or for empty text.
- **Two-level cache**: GUP-444's cached `Resolved` becomes `base`. When `hover`
  maps to tooltip text, `resolve` returns a composed `Resolved`: `base` cloned,
  plus the overlay items, plus `layout.tooltip`. The composed result is cached
  under `(base generation, hover)` and rebuilt only when either changes. When
  `base` is rebuilt while hovering, re-anchor with `pick_anchor`. If that is
  `None`, clear `hover` silently (no `Leave`).
- **Tooltip box**: `size = theme.tooltip_size` (12). Line pitch
  `(1.4 × size).round()`. Padding 6. Each line is a `TextRun` with anchor
  `(HAlign::Start, VAlign::Top)` at `(box.x + 6, box.y + 6 + i × pitch)`. Width
  `= 12 + max line width`. Height
  `= 12 + (n − 1) × pitch + cap_height + descent` (from `measure`). Round both
  up to whole pixels.
- **Placement** (canvas `W × H`, anchor `a`, gap 8): `x = a.x + 8`, and if
  `x + w > W − 1` then `x = a.x − 8 − w`. `y = a.y − 8 − h`, and if `y < 1` then
  `y = a.y + 8`. Then clamp `x` to `[1, W − 1 − w]` and `y` to `[1, H − 1 − h]`
  (low bound wins if the box is too big), and round.
- **Drawing**: one `ItemKind::Rects` with two `RectPrim`s, the box in
  `tooltip_border` and the box inset by 1 px in `tooltip_background`. Then one
  `ItemKind::Text`. Both at `pub const Z_TOOLTIP: i32 = 300` with no clip.
  `TextRole::Tooltip` is new.
- **Theme** (new fields): `tooltip_background: Color::WHITE`,
  `tooltip_border: Color::hex(0x999999)`, `tooltip_text: Color::hex(0x333333)`,
  `tooltip_size: Px(12.0)`.
- **Layout**: a new field `pub tooltip: Option<Rect>`. Tooltip runs are not
  added to `texts`, so overlap checks on guides are unaffected.
- `Event` goes in the prelude (handlers name it). `EventInfo` and `RowId` go at
  the crate root.

## Escalation Triggers

- Storing handlers makes `Selection` not `Send + Sync` on native, or needs a
  `Layer` bound change.
- The overlay needs a new scene item kind, shader or target change, beyond
  `Rects` and `Text`.
- A hover-only resolve writes `Uniform` or `Column` bytes.
- A browser scene or existing golden changes.

## Out of Scope

- Drag pan, brush, pinch and keyboard: RFC-001 S9 and later.
- Tooltips that follow the pointer, rich or HTML tooltips, and a default tooltip
  without `.tooltip()`: not planned.
- Host wiring: GUP-448 (native window) and GUP-449 (web canvas).

## Technical Tasks

- [ ] `Event`, `EventInfo`, `Selection::on`, `Selection::tooltip`, and
      `Layer::dispatch`/`tooltip_text`.
- [ ] The hover/press state machine in `Plot::handle`, and `Plot::hovered`.
- [ ] The overlay builder (pure: text sizes and canvas in, rects and runs out)
      with unit tests for placement, then the composed-resolve cache.
- [ ] Theme fields, `TextRole::Tooltip`, `Z_TOOLTIP`, `Layout::tooltip`, and the
      `SELECTION_METHODS` entries.
- [ ] `tests/events.rs`, the tooltip golden (`GUP_BLESS=1` once, then read it),
      and the placement tests.

## Dependencies

### Prerequisite Stories

- GUP-444: RFC-001 S8a cached `resolve` and `Chart::handle` 📋 — `handle`, the
  cache.
- GUP-445: RFC-001 S8b CPU picking 📋 — `pick_row`, `pick_anchor`.

### Enables Stories

- GUP-448 (S8e `GupApp` and hello world) — the hello world's tooltip.
- GUP-449 (S8f web canvas) — hover and click in a browser.

## Testing Strategy

- **Unit tests**: the placement function (corners, oversize, rounding).
- **Integration tests**: `tests/events.rs` (AC1–AC3) with synthetic `Input`.
- **Visual validation**: the tooltip golden, read by the implementer.
- **Performance**: `mask perf-budget`. Hover-only resolves are asserted to write
  no `Uniform` or `Column` bytes.

## Success Metrics

- [ ] Every AC1 sequence produces exactly the listed handler calls.
- [ ] Hover changes cost no full resolve.

## Risk Assessment

- **Medium**: event order across `Leave` and `Enter`, and the press/slop rules,
  are easy to get subtly wrong. _Mitigation_: AC1 asserts exact sequences.
- **Low**: the composed scene clones `base` (the item `Vec` and `Arc`s) on each
  hover change. _Mitigation_: there are tens of items, and only hover changes
  pay for it.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
