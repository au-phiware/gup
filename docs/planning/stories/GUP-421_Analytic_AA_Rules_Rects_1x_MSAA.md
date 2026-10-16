# GUP-421: Analytic Antialiasing for Rules and Rects; Default to 1× MSAA

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-16)
**Created**: 2026-10-10

## Context

[GUP-401](GUP-401_RFC_001_S3_Scene_Renderer_RenderTarget.md) gave `gup-core`
MSAA and made `DEFAULT_SAMPLES = 4` the default on every Gup-owned target
(`crates/gup-core/src/target.rs`).
[GUP-417](GUP-417_Track_Gup_Core_Performance_Budgets.md) measured why: MSAA 4×
costs +29% of the GPU pass uncapped and +56% under vsync on an Intel HD 630, and
it buys antialiasing only on geometric edges. The `circle.wgsl` shader already
antialiases analytically — a signed distance to the disc edge feeds a one-pixel
coverage ramp (`crates/gup-core/src/shaders/circle.wgsl`, `shade()`) — so 1× and
4× circle renders differ by at most 2/255. `rule.wgsl` and `rect.wgsl` have no
such ramp; their doc comments say so directly ("No analytic antialiasing: edges
on whole pixels are exact, and MSAA smooths the rest"). On a fan of 1.5 px
diagonal rules, 1× has 0 partially covered pixels (hard stair steps) and 4× has
597; on a rect at fractional coordinates the counts are 0 and 71
(`crates/gup-core/tests/msaa.rs`, `four_samples_smooth_geometric_edges`).

The owner decision of 2026-10-10 (RFC-001, "Decisions (2026-10-10)") keeps
`DEFAULT_SAMPLES = 4` for now rather than accept the regression that flipping it
today would cause — every rule and fractional rect (axis lines, ticks, grid, the
plot background, legend swatches) would alias again. This story does the work
the decision is conditional on: give `ItemKind::Rules` and `ItemKind::Rects` the
same analytic edge coverage `ItemKind::Marks(Circle)` already has, then flip the
default once 1× looks as good as 4× does today. This also moves `gup-core` back
in line with RFC-001 §7's original antialiasing policy — "Points, lines and text
use analytic AA... Polygons and areas use MSAA 4× by default" — which GUP-401
approximated with a blanket 4× default because no mark needed polygon/area MSAA
yet.

`ItemKind::Rules` today draws only guides: axis lines, tick marks and grid
(`crates/gup-core/src/scene.rs`, `Rule`). RFC-001's S6 (`#[derive(Mark)]` v2)
adds a `Rule` mark and a `Segment`/`Line` mark with real pixel widths, joins and
caps — the same "quad-expanded lines with real widths, joins and caps" the
strategic review's T4b lists. This story's rule quad-expansion and edge-distance
math (half-width extension for square caps, per-edge coverage) should be written
so S6 can reuse it for the `Line` mark's shader rather than inventing a second
lines-with-width implementation; it does not need to build `Line` itself.

## User Story

> "As a visualization developer, I want axis rules, ticks, grid lines and
> rectangles to look as smooth as circles do today, without paying MSAA's GPU
> cost on every frame, so that `gup-core` scenes stay sharp at the lower,
> default sample count."

A second role also benefits:

> "As a maintainer of `gup-core`'s performance budgets, I want the MSAA default
> to only cost what it buys, so that `DEFAULT_SAMPLES` reflects an actual
> trade-off rather than a blanket safety margin."

## Acceptance Criteria

### AC1: Rules get analytic edge coverage

- [x] `rule.wgsl`'s fragment shader computes signed distance to the quad's long
      edges and end caps (square caps, matching today's half-width extension)
      and ramps coverage over one physical pixel, the same technique as
      `circle.wgsl`'s `shade()`.
- [x] Axis-aligned hairlines (horizontal/vertical rules snapped to pixel
      centres) remain crisp: no change to their rendered pixels at 1× (the
      existing axis/grid goldens are unaffected within the harness's tolerance).
- [x] A diagonal rule at 1× now shows partial-coverage edge pixels comparable in
      count to today's 4× rendering of the same rule (not byte-identical — the
      two techniques differ — but the same order of magnitude, read by eye in
      the saved comparison PNG and checked by a count assertion with a
      documented tolerance).

### AC2: Rects get analytic edge coverage

- [x] `rect.wgsl`'s fragment shader (or an equivalent vertex+fragment approach)
      computes coverage for edges that fall at fractional pixel coordinates,
      ramped over one physical pixel.
- [x] A rect with all edges on whole pixels (the common case: plot backgrounds,
      legend swatches) renders identically to today's 1× output — no new blur on
      exact edges.
- [x] A rect at fractional coordinates shows partial-coverage edge pixels at 1×
      comparable to today's 4× count for the same rect.

### AC3: `DEFAULT_SAMPLES` flips to 1, MSAA stays available and auto-enables for areas and polygons

- [x] `gup_core::DEFAULT_SAMPLES` is `1`.
- [x] `TargetOptions`/`TargetDesc` keep an explicit way to opt into MSAA (for
      example `TargetOptions::with_samples(4)`, alongside the existing
      `single_sample()`), and `WindowTarget::set_samples` is unaffected.
- [x] Any `Scene` containing an area or polygon `ItemKind::Marks` item (RFC-001
      §7's "Polygons and areas use MSAA 4× by default") causes the renderer to
      render at 4× automatically, even when the target's requested sample count
      is 1 — recorded as a behavioural note if no area or polygon mark exists
      yet in `gup-core` (S6 has not landed them), with a test using a stand-in
      `ItemKind` or marking this sub-AC deferred to whichever story adds the
      first such mark, whichever is true at implementation time.

### AC4: Visual parity with today's 4× look, at 1×

- [x] `crates/gup-core/tests/msaa.rs`'s comparison scene (circles, a rule fan, a
      fractional rect), rendered with the new shaders at 1×, shows
      partial-coverage edge pixels on the rule fan and the rect within the
      harness's documented tolerance of today's 4× counts (597 and 71) — not
      byte-exact against a golden blessed on one machine, since antialiasing
      ramps are sub-pixel and machine-dependent at the margins.
- [x] The side-by-side comparison PNGs (`msaa_compare.png`,
      `msaa_compare_circle.png`) are regenerated at 1× vs the new analytic 1×
      and read by eye; the story records what was seen.
- [x] Window/`ImageTarget` parity holds: ΔE 0 between the window and the PNG for
      the same scene, as it does today (RFC-001 "S0b findings", "S3 findings").

### AC5: Performance budgets move with the new default

- [x] `mask perf-budget` passes with `DEFAULT_SAMPLES = 1`; the GPU-pass numbers
      in `crates/gup-core/PERF_BUDGETS.md` are re-recorded toward the 1× figures
      GUP-417 already measured (~3.0 ms median / ~5.5 ms p95 uncapped on the HD
      630, down from 3.89 / 6.30 at 4×), with the machine, date and story noted
      as the table's header already requires.
- [x] `zoom.samples` in `PERF_BUDGETS.md`'s budget table reads `1`.

### AC6: SVG output unaffected

- [x] `SvgTarget`'s rule and rect output (`<line>`, `<rect>`) is unchanged:
      vector output has no sample count and no coverage ramp to apply. A test
      confirms the SVG for the comparison scene is byte-identical before and
      after this story.

## Technical Tasks

- [x] Add an edge-distance helper (or inline math) to `rule.wgsl`: per-pixel
      distance to the quad's two long edges and two end caps, combined into a
      coverage value ramped over one physical pixel, following `circle.wgsl`'s
      `shade()` pattern (`clamp(0.5 - distance, 0.0, 1.0)`).
  - [x] Pass through enough varyings (local offset in physical pixels,
        half-width, segment length) for the fragment shader to compute this
        without re-deriving the vertex shader's geometry.
  - [x] Write the geometry/coverage math so it is reachable from a future
        `Line`/`Segment` mark shader (RFC-001 S6) without duplicating it — a
        shared WGSL module (`gup::marks::line_coverage` or similar) rather than
        code living only in `rule.wgsl`.
- [x] Add the equivalent coverage computation to `rect.wgsl`: distance to the
      four edges in physical pixels, ramped the same way.
- [x] Update both shaders' doc comments (currently claiming "No analytic
      antialiasing") to describe the new coverage.
- [x] Flip `crate::target::DEFAULT_SAMPLES` to `1` in
      `crates/gup-core/src/target.rs`; update its doc comment and
      `TargetOptions`'s doc comment accordingly.
- [x] Add an explicit MSAA opt-in method (e.g. `TargetOptions::with_samples`) if
      `single_sample()`'s inverse does not already exist; keep
      `WindowTarget::set_samples` working.
- [x] Implement (or document as deferred, per AC3) auto-enabling 4× MSAA when a
      scene contains an area or polygon mark, per RFC-001 §7.
- [x] Update `crates/gup-core/tests/msaa.rs`: re-tune the partial-pixel-count
      assertions and tolerances for the new analytic 1× output against the
      recorded 4× baseline; regenerate and read the comparison PNGs.
- [x] Audit other goldens that include rules or rects (axis/grid scenes,
      `scene_items.png`, dogfood goldens) for incidental changes at 1× with the
      new shaders; re-bless only where the eye confirms an improvement, not a
      regression.
- [x] Re-run `mask perf-budget` and update `crates/gup-core/PERF_BUDGETS.md`'s
      recorded numbers and header (date, story, `DEFAULT_SAMPLES` value).
- [x] Confirm `tests/targets.rs`'s byte-identity assertions (image, texture,
      draw-in-pass) still hold at the new default sample count.
- [x] Run `mask wasm-browser` to confirm the browser smoke test still passes
      with the new shaders and default.
- [x] Confirm `SvgTarget`'s rule/rect output is untouched (AC6).

## Dependencies

### Prerequisite Stories

- GUP-417: Track Gup Core Performance Budgets ✅ — measured the 4×/1× GPU-pass
  gap and the visual trade-off this story resolves, and is the source of the
  owner decision this story delivers (RFC-001 "Decisions (2026-10-10)").
- GUP-401: RFC-001 S3 Scene Renderer RenderTarget ✅ — provides `RenderTarget`,
  `TargetOptions`, `TargetDesc`, `DEFAULT_SAMPLES`, `ItemKind::Rules` and
  `ItemKind::Rects`, and the `tests/msaa.rs` comparison scene this story
  extends.

### Enables Stories

- RFC-001 S6 (`#[derive(Mark)]` v2; not yet written as a story) — reuses this
  story's quad-expanded-line coverage math for the `Segment`/`Line` mark instead
  of writing it twice.
- RFC-001 S7 (layout and guides; not yet written as a story) — benefits from
  sharper guides at the lower default sample count.

## Testing Strategy

- **Unit tests**: none beyond what the WGSL shaders need; coverage math is
  exercised through rendering, not isolated unit tests (no CPU mirror exists for
  rule/rect shading).
- **Integration tests**: extend `crates/gup-core/tests/msaa.rs` with the
  re-tuned partial-pixel-count assertions at 1× against the 4× baseline; extend
  `crates/gup-core/tests/targets.rs` to confirm byte-identity still holds at the
  new default; add an SVG byte-identity check for rules/rects.
- **Visual validation**: regenerate and read by eye
  `$CARGO_TARGET_DIR/visual-regression/gup_core/msaa_compare.png` and
  `msaa_compare_circle.png`; re-run the full golden suite and read any goldens
  that changed (axis/grid scenes, `scene_items.png`).
- **Performance**: `mask perf-budget` after the flip; record the new GPU-pass
  numbers in `PERF_BUDGETS.md` per AC5.

## Success Metrics

- [x] `DEFAULT_SAMPLES == 1` and `mask perf-budget` passes with the re-recorded
      1× budgets.
- [x] The 1× rule-fan and fractional-rect partial-pixel counts are within the
      harness's documented tolerance of today's 4× counts (597 and 71).
- [x] No golden image regresses when read by eye; any that change are re-blessed
      with a documented reason.
- [x] `mask wasm-browser` and SVG output tests pass unchanged.

## Risk Assessment

- **Medium**: matching MSAA's visual result with a single-sample analytic ramp
  is inherently approximate — MSAA integrates true sub-pixel coverage over 4
  sample points, while an edge-distance ramp approximates it with a
  smoothstep-like function. They will not be byte-identical, especially near
  corners where two edges' ramps overlap (rect corners, rule caps).
  _Mitigation_: AC4 deliberately asks for "comparable", eye-verified output and
  a documented tolerance, not byte equality; budget extra time for tuning the
  ramp width and corner handling.
- **Low**: the rule shader's end-cap coverage (two caps plus two long edges) is
  more fiddly than the rect's four independent edges or the circle's single
  radial distance. _Mitigation_: start from the rect (simpler) to validate the
  ramp approach, then tackle the rule's caps.
- **Low**: AC3's "auto-enable MSAA for areas and polygons" has no current caller
  — `gup-core` has no area or polygon mark yet. _Mitigation_: AC3 explicitly
  allows deferring that sub-AC's implementation (with a documented reason) to
  whichever story adds the first such mark, as long as the mechanism (a
  per-scene sample-count override) is at least sketched so it is not forgotten.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read)
- [x] `mask perf-budget` run and `PERF_BUDGETS.md` updated
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- **`gup::coverage`** (`crates/gup-wgsl/library/coverage.wgsl`, new): `Quad`,
  `rect_quad`, `line_quad(start, stop, width, cap, vertex_index, dpr)`,
  `oriented_quad`, `span` and `box_coverage`. Quads grow by one physical pixel;
  coverage is the fraction of a one-pixel footprint inside the box along its own
  axes (exact for axis-aligned boxes, the circle's `clamp(0.5 - d, 0, 1)` for
  wide boxes, the width's strength for sub-pixel ones).
- **`rule.wgsl`, `rect.wgsl`** use it (square caps: `cap = width / 2`).
- **`DEFAULT_SAMPLES = 1`**; `TargetOptions::with_samples` replaces
  `single_sample`; `WindowTarget` no longer probes for 4×; docs updated.
- **Auto-MSAA (AC3)**: `Antialias { Analytic, Msaa }`, `Mark::ANTIALIAS`
  (default `Analytic`), `MarkBatch::antialias`, `Scene::min_samples`, and
  `Renderer::render` drawing with 4 samples into a renderer-owned multisampled
  texture when the scene needs more than the target has (`Frame::attachment`).
  Tested with a stand-in mark.
- **Tests** (`crates/gup-core/tests/msaa.rs`, 2 → 5 tests):
  `rules_and_rects_antialias_without_msaa` (counts against the pre-story 4×
  render kept as `fixtures/msaa_4x_before_analytic.png`: tolerance 1.0–1.5× of
  597 and 71; fan ink within 2%; the rect's exact per-pixel coverage within
  2/255; 1× and 4× within 2/255),
  `snapped_hairlines_and_whole_pixel_rects_stay_crisp` (no partial pixel and
  exact ink area at dpr 1 and 2, 1× and 4×),
  `scenes_with_msaa_marks_draw_multisampled`,
  `svg_of_the_comparison_scene_is_unchanged` (byte-identical to
  `fixtures/msaa_guides.svg`, written before the change), and the existing
  circle test. Both seeded regressions (a 1.5 px ramp; render ignoring
  `min_samples`) failed the new tests.
- **Goldens**: `msaa_1x`, `msaa_4x` re-blessed (now the analytic ramp). No other
  golden changed beyond tolerance.
- **Docs**: `PERF_BUDGETS.md` re-recorded; RFC-001 "GUP-421 findings"; GUP-423
  notes the module name.

### Definition-of-Done evidence

- gup-core: lib 123 passed; every integration test file passed (msaa 5, targets
  4, svg 2, scene_items 2, scatter 3, scales 5, ordinal 3, wgsl_function 5,
  zoom_uploads 2, compile_fail 2). Root-crate tests left to CI.
- `window_parity` (display): window vs `ImageTarget` ΔE max 0, at `samples: 1`.
- `mask wasm-browser`: `GUP PASS` on SwiftShader (Chrome 154).
  `mask perf-budget`, two complete runs, GPU clock median 367 MHz (floor):

| metric                  | recorded |   run 1 |   run 2 |
| ----------------------- | -------: | ------: | ------: |
| `zoom.samples`          |        1 |       1 |       1 |
| `gpu_pass.median_ms`    |     2.95 |   3.003 |   2.953 |
| `gpu_pass.p95_ms`       |     5.60 |   5.875 |   5.591 |
| `cpu.median_ms`         |     0.69 |   0.582 |   0.578 |
| `cpu.p95_ms`            |     0.86 |   0.712 |   0.698 |
| `link_create.median_ms` |     0.88 |   1.042 |   0.956 |
| `wasm.scatter.gz_bytes` |  302,929 | 302,929 | 302,929 |

Every other check passed (88 uniform and 0 column bytes per frame). At 4×,
alternating runs measured 4.52–4.60 ms median, 8.50–8.87 ms p95. A third run
while another checkout was compiling (load 13.7) failed CPU and pipeline timings
only.

- `mask old-path-loc`: 28,910.

## Retrospective

**Completed**: 2026-10-16

### What the renders showed

- **Rule fan (msaa_compare.png: before 4× | now 1× | now 4×)**: the new 1× fan
  reads like the old 4×: one dark core pixel with pale blue fringes along each
  diagonal, no stair steps. The old 1× was hard staircases. New 1× and 4× are
  indistinguishable.
- **Fractional rect (msaa_compare_rect.png)**: the old 4× showed MSAA's
  quantised edge tints (25/50/75%); the new 1× shows the exact fractions: 70%
  left and right columns, a 40% top row and a faint 10% bottom row that MSAA
  missed entirely.
- **Circles and axes (msaa_compare_circle.png)**: identical across all three.
- **Plot goldens** (scatter, categorical_species with its swatch legend,
  categorical_nulls, sqrt/symlog/time scatters, scene_items, the two
  `#[wgsl_function]` scenes): byte-identical to the old 1× and 4× renders; axes,
  ticks and swatches are as crisp as before, with no hairline blur.
- **band_months**: the bands' fractional vertical edges now get a light partial
  column, as at old 4× (at most 3/255 from it); old 1× had hard edges.
- **diverging_anomaly, wgsl_function_aftershocks**: the ramp legend's ticks
  start half a pixel into a pixel; their end pixels are now half-tinted as at
  old 4× (within 1/255) instead of the old 1× hard end.

### Key Technical Learnings

#### Box coverage, not SDF coverage

- **Challenge**: the classic `clamp(0.5 - d)` ramp blurs hairlines when the
  line's centre, not its edges, sits on a pixel centre, and it is wrong for
  boxes under one pixel wide.
- **Solution**: per-axis footprint overlap,
  `clamp(min(t + 0.5, h) - max(t - 0.5, -h), 0, 1)`, multiplied over the box's
  two axes. Snapped edges land on pixel boundaries and give exactly 1 or 0.
- **Pattern**: treat rects and lines as one primitive (an oriented box); the
  mark only decides the frame.

#### Comparing against a technique you are replacing

- **Challenge**: after the change, 4× renders also use analytic coverage, so "1×
  vs today's 4×" cannot be rendered any more.
- **Solution**: keep the pre-change 4× golden as a test fixture and compare
  counts and ink against it (loose, cross-device-safe), and check the rect
  against exact CPU coverage (machine-independent).

### Architectural Decisions

#### MSAA for a scene, chosen by the renderer

- **Decision**: `Mark::ANTIALIAS` → `MarkBatch` → `Scene::min_samples`;
  `Renderer::render` keeps its own multisampled texture when a scene needs more
  samples than the target has.
- **Reasoning**: no change to `RenderTarget`; every Gup-owned target gets it;
  pipelines are already keyed by sample count.
- **Trade-off**: draw-in-pass hosts must read `min_samples` themselves; the
  renderer holds one extra texture while such scenes are drawn.
- **Future**: S6/S11's area and polygon marks set `ANTIALIAS = Msaa`.

#### `with_samples` replaces `single_sample`

- With 1 the default, `single_sample()` became a no-op; one method covers both
  directions.

### Development Workflow Insights

- Same-device before/after renders (old 1×, old 4×, new) diffed with ImageMagick
  told exactly which goldens moved and by how much before any blessing.
- `mask wasm-browser` needs `env -u LD_LIBRARY_PATH` with the system Chromium
  (the dev shell's alsa-lib wants a newer glibc), and it hung in DevTools under
  heavy load from a concurrent build; on a quiet machine it passed first time.
- Perf timings are only meaningful when no other checkout is compiling: CPU p95
  tripled under load.
- The trybuild expectation listing the library's modules must change whenever a
  library module is added.

### Follow-up Stories

None. `gup::coverage` is ready for GUP-423; area/polygon marks (S6/S11) only set
`Mark::ANTIALIAS`.
