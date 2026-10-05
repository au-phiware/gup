# GUP-405: gup-text OpenType Shaping (Kerning and Tabular Figures)

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-06

## Context

[GUP-400](GUP-400_RFC_001_S2_Extract_Gup_Text.md) (RFC-001 S2) extracted
`gup-text` and made Inter Regular its bundled default face. Its layout gets
glyph advances and kerning from fontdue, which reads kerning only from the
legacy `kern` table. Inter 4 has no `kern` table and keeps its kerning in GPOS,
so every run is laid out unkerned. GUP-400's probe found that `horizontal_kern`
returns `None` for "AV", "To" and "11".

Inter's default digits are also proportional. At 16 px, "1" advances 6.5 px and
"0" advances 10.1 px. Its tabular figures are the `tnum` OpenType feature, a
GSUB substitution that fontdue cannot apply. The strategic review's T4a asked
for "a neutral UI face with tabular figures". Proportional digits make
right-aligned y-axis labels ragged (`1M` vs `500k`), and their widths change as
a zoom changes the tick values. That widens or narrows the margin and nudges the
plot rect from frame to frame.

`rustybuzz` (a pure-Rust HarfBuzz port, `ttf-parser`-based) shapes text into
positioned glyph indices with GPOS kerning and any requested features. fontdue
can rasterise those indices (`rasterize_indexed`, `metrics_indexed`), so the
bitmap atlas and the draw path stay as they are.

## User Story

> "As a Gup user reading a chart, I want tick labels set in tabular figures and
> titles properly kerned, so that numbers line up in columns, axis margins stay
> steady while I zoom, and text looks like a well-set UI face."

## Acceptance Criteria

### AC1: Runs are shaped

- [ ] `gup-text` lays out runs with `rustybuzz`, from the same font bytes
      fontdue rasterises. The atlas is keyed by glyph index and size, not by
      `char`.
- [ ] GPOS kerning applies. A unit test shows that "AV" (or "To") in Inter at 16
      px is narrower shaped than the sum of its advances.
- [ ] `measure`, `ink_bounds` and `layout` all use the shaped advances, so
      measurement and drawing still agree to the pixel. GUP-400's
      `draw_into_pass` test (ink only inside `ink_bounds`) still passes.

### AC2: Tabular figures for numeric labels

- [ ] `Run` (or a `TextStyle`-level option) can request OpenType features. Tick
      labels in `gup-core` request `tnum`.
- [ ] A test shows that "0" through "9" have identical advances when `tnum` is
      requested, and that "1000" and "1111" measure the same width.

### AC3: Golden output (user-visible)

- [ ] `tests/golden/gup_core/scatter.png` is re-blessed and checked by eye. The
      y labels align digit-for-digit and the title is kerned. The retrospective
      describes what changed.

## Technical Tasks

- [ ] Add `rustybuzz` to `gup-text`. Keep one parsed `rustybuzz::Face` next to
      the fontdue face in `Font`, built once per process like the fontdue face.
- [ ] Replace the char walk in `layout.rs` with shaping. Map clusters back to
      text for ink bounds.
- [ ] Key the atlas by `(glyph index, size bits)` and rasterise with
      `rasterize_indexed`.
- [ ] Add a feature list to `Run`, and set `tnum` on `gup-core`'s tick-label
      runs.
- [ ] Measure the shaping cost of the S0b zoom (26 labels + title per frame) and
      record it.

## Dependencies

### Prerequisite Stories

- GUP-400: RFC-001 S2 — Extract `gup-text` ✅

### Enables Stories

- RFC-001 S7 (layout and guide emitters): stable label widths make the
  tick-density rule ("no two labels closer than one em") easier to hold.

## Testing Strategy

- **Unit tests**: kerning pair narrower than the summed advances; `tnum` digits
  equal width; atlas keyed by glyph index.
- **Integration**: `crates/gup-text/tests/draw_into_pass.rs` still passes
  unchanged; `gup-core`'s `scatter_png` passes after the re-bless.
- **Visual**: the re-blessed golden is read by eye.

## Success Metrics

- [ ] Tick labels align digit-for-digit in the golden image.
- [ ] Resolve time of the S0b zoom bench rises by less than 0.1 ms per frame
      (release).

## Risk Assessment

- **Medium**: wasm size. rustybuzz adds code to every target. _Mitigation_:
  measure with S3's WASM size check. Keep the change if gup-core stays inside
  the ≤ +400 KB gz budget.
- **Low**: two parsers over one font. _Mitigation_: both read the same `&[u8]`,
  and the bundled face is static. A test asserts glyph counts match.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-text -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Golden PNG verified by eye and described in the retrospective
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
