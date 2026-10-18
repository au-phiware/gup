---
implementer: sonnet
implementer-reason:
  the enum, the WGSL, the uniform layout and every call site are fixed in
  Decisions; it extends GUP-458's brushed glue variant
---

# GUP-477: Grey Unbrushed Rows (`Theme::unbrushed`)

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-19

## Context

GUP-458 dims rows outside a brush by multiplying every colour channel's alpha by
`Theme::unbrushed_opacity` (default 0.3). Faded rows keep their hue. Linked
brushing usually wants the opposite: rows outside the brush **greyed**, so the
selection is the only colour on screen.

Dogfood task 4 (`dogfood/src/bin/t4_linked.rs`) checks exactly that ("left view
outside the brush: grey only", at most 20 saturated pixels). GUP-465 measured
13,545–15,197 saturated pixels with fades of 0.1 and 0.3, so it set
`unbrushed_opacity: 0.0` as a stopgap. That hides the unbrushed rows instead of
greying them, and GUP-465 flagged it for review.

This story replaces the `f32` with a style enum, `Fade` or `Grey`, keeps
`Fade(0.3)` as the default (so every existing image is unchanged), and moves t4
onto `Grey` with a check that the greyed rows are visible.

## Read First

- [GUP-458](GUP-458_RFC_001_S9g_Dim_Unbrushed_Rows.md) "Decisions" and
  "Implementation Summary" (the brushed glue variant, `brush_dim`, the bits).
- `crates/gup-core/src/theme.rs` (whole, 138 lines).
- `crates/gup-core/src/shader/glue.rs`: `GlueSpec::brushed` (lines 65–69),
  `BRUSH_DIM_FIELD` (line 262), the field push (lines 477–492) and the brushed
  WGSL (lines 656–670).
- `crates/gup-core/src/selection.rs`: `unbrushed` (lines 147–148, 185),
  `brush_dim()` (lines 491–498), `prepare` (lines 1055–1105, the `Encodings`
  writes).
- `crates/gup-core/src/render.rs` `write_brush_dim` (lines 1058–1073).
- `crates/gup-core/src/plot.rs` line 1773 (`brushable`).
- `crates/gup-core/tests/events.rs` `brush_golden` (lines 647–710) and
  `render_dimmed` (lines 999–1008).
- `dogfood/src/bin/t4_linked.rs` lines 40–64, `dogfood/src/suite.rs` `fn t4`
  (lines 959–1006), `dogfood/README.md` "Task 4" (lines 133–142).

## User Story

> "As a developer building linked views, I want rows outside a brush drawn in
> grey, so that the brushed rows are the only colour on the chart while the rest
> stay visible for context."

## Acceptance Criteria

### AC1: The API

- [ ] `Theme::unbrushed_opacity` is gone; `Theme::unbrushed: Unbrushed` replaces
      it, with the enum in Decisions, exported as `gup_core::Unbrushed` and in
      the prelude.
- [ ] `Theme::default().unbrushed == Unbrushed::Fade(0.3)`, asserted in
      `theme.rs`'s `default_is_the_old_styling`.
- [ ] A doctest on `Unbrushed` builds a plot with
      `Theme { unbrushed: Unbrushed::Grey(Color::hex(0xbfbfbf)), ..Theme::default() }`
      and `Drag::Brush`, and compiles from outside the crate (doctests are
      external).
- [ ] `git grep -n unbrushed_opacity -- ':!docs/planning'` finds nothing.

### AC2: Fade is unchanged

- [ ] With `Fade(_)` the generated glue is byte-identical to before: the
      existing glue unit tests in `shader/glue.rs` pass without edits, and
      `tests/golden/gup_core/brush.png` is not re-blessed.
- [ ] `Fade(1.0)` turns dimming off (no brushed glue), as `1.0` did.
- [ ] GUP-458's upload counts still hold: a brush appearing or clearing writes
      the bits plus one 4-byte uniform write, for `Fade` and `Grey` alike.

### AC3: Grey renders grey

In a new `brush_grey_golden` test in `crates/gup-core/tests/events.rs`, built
like `brush_golden` (same rows, radius and brush) with
`Unbrushed::Grey(Color::hex(0xbfbfbf))`:

- [ ] The centre pixel of each unbrushed dot is within 1 per channel of
      `#bfbfbf`; each brushed dot's centre equals its undimmed render exactly.
- [ ] With the layer at `opacity(0.5)`, an unbrushed centre is within 2 of
      `#bfbfbf` at 0.5 over white (`#dfdfdf`).
- [ ] A circle with a stroke: under `Grey`, both its fill and its stroke pixels
      are within 2 of the grey (every `Role::Color` channel is replaced).
- [ ] `Unbrushed::Grey(Color { a: 0.5, ..Color::hex(0xbfbfbf) })` halves the
      unbrushed alpha (centre within 2 of `#dfdfdf` on an opaque layer).
- [ ] Clearing the brush restores the undimmed image byte for byte.
- [ ] The golden `tests/golden/gup_core/brush_grey.png` is blessed, and the
      implementer reads it: the two brushed dots are the usual blue, the other
      three are mid grey and fully visible.

### AC4: Dogfood task 4 greys, not hides

- [ ] `t4_linked`'s left chart uses `Unbrushed::Grey(Color::hex(GREY))` (its
      existing `GREY` const, `0xbfbfbf`).
- [ ] `suite.rs` `t4` keeps "left view outside the brush: grey only" exactly as
      is (`NoColour`, `max_px: 20`) and gains, on the same region, "left view
      outside the brush: rows greyed, not hidden" as
      `Measure::Grey { region: Region::new(0.125, 0.3, 0.215, 0.65), min_px: 500 }`.
- [ ] `xvfb-run -a ./dogfood/run_all.sh` reports no FAIL and no XPASS; paste the
      t4 lines into the Implementation Summary. Read `t4_linked.png`: the left
      view shows grey points outside the brush and coloured ones inside.
- [ ] `dogfood/README.md`'s task 4 paragraph describes the grey style instead of
      the opacity-0 workaround.

### AC5: Every other user

- [ ] `git grep -nE "unbrushed|brush_dim" -- '*.rs' '*.js' '*.mjs' '*.html'` is
      reviewed and every hit outside the files above is listed in the
      Implementation Summary with what changed (expected: `tests/events.rs`'s
      `render_dimmed` and `brush_golden`, `wasm-size/scatter`'s brush step needs
      nothing, as it uses the default).

## Decisions

- **The enum** (in `theme.rs`):

  ```rust
  /// How rows outside a brush are drawn while one exists.
  #[derive(Copy, Clone, Debug, PartialEq)]
  pub enum Unbrushed {
      /// Multiply every colour channel's alpha by this opacity
      /// (0.0..=1.0). `Fade(1.0)` turns dimming off.
      Fade(f32),
      /// Draw every colour channel in this colour; its alpha multiplies
      /// the mark's alpha.
      Grey(Color),
  }
  impl Default for Unbrushed { fn default() -> Self { Unbrushed::Fade(0.3) } }
  ```

  Reason: the names the orchestrator proposed; `f32` matches the builders'
  `opacity(f32)`. `Grey` takes any colour, so a theme can "grey" in a tint.

- **Theme field**: `pub unbrushed: Unbrushed` replaces `unbrushed_opacity`, same
  doc position. Breaking changes are approved (strategic review Decision 2).
- **When the glue is brushed**: `self.drag == Drag::Brush` and the style dims,
  where `Fade(o)` dims iff `o < 1.0` and `Grey(_)` always dims. Add
  `pub(crate) fn dims(&self) -> bool` on `Unbrushed`.
- **Glue**: `GlueSpec::brushed: bool` becomes `brushed: Option<BrushStyle>` with
  `pub(crate) enum BrushStyle { Fade, Grey }`.
  - `Fade`: exactly today's glue (signature suffix "brushed").
  - `Grey`: signature suffix "brushed grey", one more `Encodings` field,
    `pub(crate) const BRUSH_GREY_FIELD: &str = "brush_grey";` of type
    `vec4<f32>` (16 bytes), pushed after `brush_dim`. The WGSL, after the same
    `brush_row`/`brush_in`/`brush_k` lines, for every `Role::Color` channel `c`:

    ```wgsl
    m.c = vec4<f32>(mix(enc.brush_grey.rgb, m.c.rgb, brush_k),
                    m.c.a * mix(enc.brush_grey.a, 1.0, brush_k));
    ```

    No identifier ends in a digit (RFC-001 "S5c findings").

- **`brush_dim` keeps its 4-byte write**: it is still "1.0 while there is no
  brush". With a brush it is `o` for `Fade(o)` and `0.0` for `Grey`, so for
  `Grey` it is a 0/1 weight and the formula above selects the grey for unbrushed
  rows and the row's own colour otherwise. `brush_grey` is written once at
  `prepare` (with the other `Encodings`, `bytemuck::bytes_of(&color)` as
  `null_color` is), never on a brush change. Reason: GUP-458's upload counts and
  the per-change write stay as they are.
- **Selection state**: `Selection::unbrushed: f32` becomes
  `unbrushed: Unbrushed` (default `Fade(1.0)`); `brush_dim()` maps it as above.
- **SVG output**: `SvgTarget` does not draw brush dimming today (GUP-467) and
  this story does not add it.
- **t4 threshold**: `min_px: 500` matches the sibling "left view inside the
  brush: colour" check (500 px in a region of the same size).

## Escalation Triggers

- Any existing glue test, golden or browser step changes under the default
  `Fade(0.3)`.
- The `Grey` formula leaves a coloured rim on stroked or antialiased edges
  (saturation above 0.25 in `NoColour`'s sense) that the AC3 stroke check
  catches.
- t4's "grey only" check fails with `Grey` (more than 20 saturated pixels), or
  its new grey check finds fewer than 500 grey pixels.

## Out of Scope

- Dimming in `SvgTarget` output: not scheduled.
- Per-layer brush styles: the style is the plot's theme.
- The tooltip check of task 3: GUP-478.

## Technical Tasks

- [ ] `Unbrushed`, the `Theme` field, prelude export, doctest.
- [ ] `GlueSpec::brushed: Option<BrushStyle>`, the grey field and WGSL.
- [ ] `Selection` state, `brush_dim()`, the `brush_grey` write at prepare;
      `plot.rs` `brushable`.
- [ ] Tests: update `render_dimmed`/`brush_golden`; add `brush_grey_golden` and
      the AC3 pixel checks; bless and read the golden.
- [ ] Dogfood t4: theme, the new check, README; run the suite.

## Dependencies

### Prerequisite Stories

- GUP-458: RFC-001 S9g Dim Unbrushed Rows ✅ — the brushed glue and bits.
- GUP-465: RFC-001 S10f Dogfood on gup-core ✅ — t4 on gup-core.

### Enables Stories

- None.

## Testing Strategy

- **Unit tests**: glue emission for `Grey` (signature, field, WGSL lines);
  `Unbrushed::dims`.
- **Integration tests**: `brush_grey_golden` and the AC3 checks in `events.rs`.
- **Visual validation**: read `brush_grey.png` and `t4_linked.png`.
- **End to end**: `xvfb-run -a ./dogfood/run_all.sh`.

## Success Metrics

- [ ] Task 4's left view shows unbrushed rows in grey, checked by two pixel
      measures.
- [ ] No existing image changes.

## Risk Assessment

- **Low**: the `mix` with a 0/1 weight is exact only if `brush_dim` is exactly
  `0.0` or `1.0` for `Grey`. _Mitigation_: AC3 checks centres within 1.
- **Medium**: trybuild `.stderr` churn. The snapshots in
  `crates/gup-core/tests/{compile_fail,derive_mark_fail,wgsl_function_fail}`
  quote line numbers in `src/selection.rs`, `src/encoding.rs` and
  `src/marks.rs`. Edits above those lines shift them. _Mitigation_: re-bless
  with `TRYBUILD=overwrite` only when the diff is line numbers; any change to
  message text is an escalation.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] `mask wasm-browser` passes (this story touches the input path's brush
      rendering)
- [ ] `mask wasm-size` run and the scatter and per-row-type figures reported in
      the Implementation Summary (the orchestrator re-records the budget)
- [ ] `mask perf-budget` table pasted (story-worker step 9)
- [ ] Rendered output verified by eye: `brush_grey.png`, `t4_linked.png`
- [ ] `mask old-path-loc` recorded (expected unchanged)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
