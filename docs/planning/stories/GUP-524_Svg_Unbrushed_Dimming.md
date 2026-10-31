---
implementer: sonnet
implementer-reason:
  a CPU mirror of the GPU's brush state shared with the vector snapshot, with
  the WGSL formula to copy and a golden parity check that fails today
touches:
  [
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/vector.rs,
    crates/gup-core/tests/theme_dark_png.rs,
    crates/gup-core/tests/events.rs,
  ]
---

# GUP-524: SVG Draws Unbrushed Rows Dimmed

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-26 (decision-complete rewrite 2026-10-31)

## Context

The GPU dims rows outside a brush. The brushed glue (`shader/glue.rs`, around
line 765) computes `brush_k = select(enc.brush_dim, 1.0, brush_in)` per
instance. For `Unbrushed::Fade` it multiplies every colour channel's alpha by
it. For `Unbrushed::Grey` it mixes the colour towards `enc.brush_grey`. The
vector snapshot (`vector.rs`, `CpuLayer`) knows nothing of the brush, so
`SvgTarget` draws every mark at full strength. GUP-553's brush golden
(`gup_core/theme_dark_brush`) therefore skips the SVG parity check
(`render_golden(.., false)` in `tests/theme_dark_png.rs`). Measured there, 668
of 16,000 4×4 blocks differ, with a maximum ΔE of 46. Any SVG exported from a
brushed chart shows something different from the screen.

The brush can change without a new snapshot. `Plot::sync_brush` writes new bits
into GPU buffers that the cached scene already holds (`Selection::write_brush`,
then `write_bits`), and a cached resolve reuses the old `MarkBatch`. A copy of
the brush taken at `prepare` would go stale, so the mirror must be shared.

## Read First

- `crates/gup-core/src/shader/glue.rs` lines 760–785: the brush formula.
- `crates/gup-core/src/selection.rs`: fields `brushable`, `unbrushed` and
  `brush` (lines 214–220); `brush_dim` and `write_bits` (lines 852–897);
  `vector` (lines 1249–1325); `prepare` lines 1776–1780 and 1836–1846 (the brush
  uniform fields); `write_brush` (lines 2005–2023).
- `crates/gup-core/src/vector.rs` lines 80–95 (`CpuLayer`), 206–228 (`colors`)
  and 250–262 (`marks`).
- `crates/gup-core/src/plot.rs` lines 1181–1201 (`sync_brush`), for reference
  only, not edited.
- `crates/gup-core/tests/theme_dark_png.rs` lines 66–140; `tests/events.rs`
  `brush_golden` and `brush_grey_golden` (lines 680–750 and 1398–1462).
- `crates/gup-core/tests/common/svg_marks.rs` (`assert_matches`,
  `MAX_BLOCK_DE`).

## User Story

> "As a developer exporting a brushed chart, I want the SVG to dim unbrushed
> rows as the window does, so that the export matches what I see."

## Acceptance Criteria

### AC1: The SVG dims as the GPU does

- [ ] For every mark the vector path writes (circle, rect, segment, line, area),
      each colour channel (`Role::Color`: `fill`, `stroke`, `color`) of a stored
      row outside the brush gets the glue's formula with `k = dim`, and a row
      inside gets `k = 1`. `Fade`: alpha × k. `Grey`: rgb = mix(grey.rgb, rgb,
      k), alpha × mix(grey.a, 1, k).
- [ ] Unit test in `vector.rs` (or `tests/svg.rs`): 3 circles, rows {1} brushed.
      Under `Fade(0.3)`, rows 0 and 2 have `fill-opacity="0.3"` and row 1 has
      none. Under `Grey(#bfbfbf)`, rows 0 and 2 are `fill="#bfbfbf"`. With the
      brush cleared, the SVG is byte-identical to the unbrushed one.

### AC2: Parity on the goldens

- [ ] `dark_theme_brush_golden` calls `render_golden(.., true)`, and the "does
      not dim" comment is gone. It passes within `MAX_BLOCK_DE`.
- [ ] `brush_golden` and `brush_grey_golden` in `tests/events.rs` call
      `svg_marks::assert_matches` on their scene. Any tooltip box there becomes
      a text region, as `theme_dark_png.rs` does it.
- [ ] Seeded failure, recorded in the retrospective: forcing `k = 1` makes
      `theme_dark_brush`'s parity check fail.

### AC3: The mirror follows a cached scene

- [ ] Integration test: resolve a brushed plot and write its SVG (dimmed). Clear
      the brush with `plot.clear_brush()`, resolve again (cached: no full
      resolve) and write the SVG of the new `Resolved::scene`: nothing dimmed.
      Brush again: dimmed again.

### AC4: Nothing else changes

- [ ] No PNG golden changes (`git diff --stat tests/golden` is empty), and every
      existing SVG test passes unchanged.
- [ ] `mask wasm-size`: scatter within 300 B gz of the Start commit (the mirror
      is behind the `svg` feature, which the WASM build leaves off).

## Decisions

- **A shared CPU mirror**, in `vector.rs` behind `#[cfg(feature = "svg")]`:

  ```rust
  /// What the GPU's brush state says, for vector targets (GUP-524).
  #[derive(Clone, Debug, Default)]
  pub(crate) struct BrushMirror {
      /// The glue dims at all (`Selection::brushable` at the last prepare).
      pub on: bool,
      /// Rows inside the brush, one bit per stored row, exactly as `write_bits` wrote them.
      pub bits: Vec<u32>,
      /// The `brush_dim` uniform: 1.0 without a brush, else `Fade`'s opacity or 0.0 for `Grey`.
      pub dim: f32,
      /// `Unbrushed::Grey`'s colour, else `None` (Fade).
      pub grey: Option<Color>,
  }
  ```

  `Selection` gains `brush_mirror: Arc<Mutex<BrushMirror>>`, created in `new`.
  `CpuLayer` gains `brush: Arc<Mutex<BrushMirror>>` (a clone of that `Arc`, set
  in `Selection::vector`). The `Arc` is shared, so a cached scene sees later
  brush writes, just as its GPU buffers do.

- **Every write the GPU gets, the mirror gets**, in `selection.rs`, each under
  `cfg(feature = "svg")`:
  - `prepare`, after `self.unbrushed = theme.unbrushed`: set `on`, `dim`
    (`self.brush_dim()`) and `grey`.
  - `write_bits`: compute the bits as now, store a copy in the mirror, then
    write them to the GPU. If there is no brush or no columns, store empty bits.
  - `write_brush`: after updating `self.brush`, set `dim` to `self.brush_dim()`,
    and empty the bits when the brush is `None` (it returns before `write_bits`
    when there is no GPU state).
- **Applying it**: `CpuLayer::marks` locks the mirror once, clones it and passes
  it to `colors`. `colors(name, needed, &brush)` applies the formula per stored
  row when `brush.on` and the channel's role is `Some(Role::Color)`, turning a
  `Colors::Const` into `Colors::Col` over all stored rows. The stored row
  `d = row(e.this, e.first + i)` is the GPU's `chunk.row_base + instance_index`.
  A row is inside the brush if its bit is set, and outside if the bit is clear
  or past the end. With `brush.on == false` or `dim == 1.0` with no grey,
  `colors` returns what it returns today, which keeps unbrushed SVGs
  byte-identical.
- **Colour maths** in straight-alpha `Color` (sRGB-encoded components, as the
  glue sees them): `mix(a, b, t) = a + (b - a) * t`. The fill opacity is still
  multiplied afterwards by `with_opacity`, as now. Multiplication commutes with
  the GPU's order.
- **Tests** reuse the goldens' scenes and `svg_marks`. No new golden.

## Escalation Triggers

- `brush_golden` or `brush_grey_golden` fails parity for a reason other than
  dimming, such as a tooltip or the brush rectangle itself: report the block
  statistics and the diff image instead of loosening `MAX_BLOCK_DE`.
- The mirror needs a lock in `vector.rs` that a `Send + Sync` bound on
  `LayerVector` rejects.
- Some path writes brush bits to the GPU other than `write_bits`, so the mirror
  can't follow it.

## Out of Scope

- Dimming in text layers (labels of unbrushed rows): GPU text is not dimmed
  either.
- A brush on user-defined marks: they have no vector snapshot (`Skipped`).

## Technical Tasks

- [ ] `BrushMirror`, `Selection::brush_mirror`, `CpuLayer::brush`.
- [ ] Mirror writes in `prepare`, `write_bits` and `write_brush`.
- [ ] `colors` applies the formula; the AC1 unit test.
- [ ] Parity on the three brush goldens; AC3 test; seeded failure.
- [ ] `mask wasm-size`.

## Dependencies

### Prerequisite Stories

- GUP-553 ✅: the `theme_dark_brush` golden.
- GUP-467 ✅ and GUP-477 ✅: the vector snapshot and the `Grey` style.

## Testing Strategy

- **Unit tests**: the formula on circles under both styles.
- **Integration tests**: three goldens with parity, and the cached-scene test.
- **Visual validation**: the parity check compares the rasterised SVG with the
  GPU image block by block. `theme_dark_brush`'s diff today is the reference
  failure.

## Success Metrics

- [ ] Every brushed golden passes the SVG parity check.

## Risk Assessment

- **Low**: SVG size grows only for brushed charts (one more `fill-opacity` per
  dimmed mark, and polylines split where the brush starts and ends).
- **Low**: a mutex per layer. _Mitigation_: it is locked once per
  `CpuLayer::marks` and once per brush write.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output: the parity checks (no PNG golden changes)
- [ ] `mask wasm-browser`: not needed (no layout, input or wasm code change);
      say so
- [ ] WASM size: `mask wasm-size` scatter gz bytes
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
