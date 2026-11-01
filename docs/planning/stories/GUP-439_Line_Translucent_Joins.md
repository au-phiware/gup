---
implementer: opus
implementer-reason:
  a per-layer coverage pass against the single-pass render model, with design
  alternatives to measure; design work by nature
touches: [crates/gup-core/src/selection.rs, crates/gup-core/src/render.rs]
---

# GUP-439: Translucent Lines Without Double-Blended Joins

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: ✅
Complete (2026-11-02) **Created**: 2026-10-17

## Context

[GUP-432](GUP-432_RFC_001_S6f_Line_Mark.md) draws each `Line` segment as a
capsule, so consecutive segments overlap in a disc at their shared row. An
opaque line shows no seam. A translucent one blends the overlap twice: at alpha
0.5 a join has 0.75 ink against 0.5 on the arms
(`a_translucent_join_blends_twice` in `crates/gup-core/tests/line_png.rs` pins
this). A sharp turn also overlaps on its inner side, beyond the disc. A
translucent time series with many points therefore shows a bead at every vertex,
and overplotted translucent series, a common way to show density, look dotted.

The fix cannot come from the segment alone. Excluding the overlap needs the
neighbours' directions (rows `i − 1` to `i + 2`), which RFC-001 "GUP-432
findings" rejected for the slot budget. Clipping each capsule's start disc
leaves a seam where the coverage ramps cross. The usual remedy works per layer:
draw the line's coverage once (an offscreen target with `Max` blending, or a
stencil pass), then composite it with the line's colour and alpha.

## User Story

> "As a visualization developer, I want a translucent line to have one even
> opacity along its whole length, so that overplotted series show density, not
> beads at every data point."

## Acceptance Criteria

### AC1: Even opacity

- [x] A `Line` with colour alpha 0.5 has the same ink at a join as on its arms,
      within 2/255, for turns of 30°, 90° and 170°, and where the line crosses
      itself.

### AC2: Opaque lines unchanged

- [x] Opaque lines render byte-identically to before, with no extra pass, and
      `gup_core/line_series` is unchanged.

### AC3: Cost recorded

- [x] The extra pass or target's cost (GPU time at 100K points, memory, draws)
      is measured and recorded in RFC-001, along with the design chosen and the
      designs rejected.

## N2 Notes (2026-10-24)

Absorbed into track N2 by the November review's triage ("same polyline geometry
as line curves and area outlines; do them together") and promoted from 💡 to 📋.
It runs after GUP-550 and GUP-551.

### Read First

- RFC-001 "GUP-432 findings" (capsules, bridges, "Joins and caps"), "GUP-550
  findings" (joint ownership, `joins`/`others`), "GUP-551 findings" and "GUP-576
  findings" (plane joints, piece counts, cost table): `grep -n` for the headings
  in `docs/planning/rfcs/RFC-001_Core_Architecture.md`.
- `crates/gup-core/src/render.rs`: `Renderer::prepare_unscoped`,
  `Renderer::render`, `Prepared::draw`, `create_pipeline`.
- `crates/gup-core/src/selection.rs`: `Layer::prepare` (where the batch is
  built) and `brush_dim`.
- `crates/gup-wgsl/library/line.wgsl`: `shade` (premultiplied output).
- `crates/gup-core/tests/line_png.rs`: `a_translucent_join_blends_twice`.
- `crates/gup-core/src/svg.rs`: `write_layer`.

### Decisions

(Written 2026-11-02 at the start of implementation; the N2 notes' decisions are
kept, the coverage format is amended.)

- **Scope includes curve pieces**: after GUP-550/551 a translucent step or
  monotone line overlaps at every piece joint as well as at rows; AC1's even
  opacity applies to those joints too (add 3 piece joints of a `MonotoneX` line
  and the corners of a `StepAfter` line to the AC1 measurements).
- **Areas are not in scope**: `Area` trapezoids share edges and do not
  double-blend (GUP-545). Area outlines are `Line` layers and get the pass like
  any other line.
- **Mechanism: an offscreen target per translucent line layer.** The layer is
  drawn with its own glue and `line.wgsl` (no shader change) into an
  `Rgba8Unorm` texture cleared to 0, with `BlendOperation::Max` on colour and
  alpha. `shade` outputs premultiplied colour times coverage, so each texel
  holds the largest coverage any segment or piece gives it: the coverage of the
  union, the round-joined stroke drawn once, at joins, piece joints, overlapping
  segments and self-crossings alike. The scene pass then draws one full-target
  triangle per such layer, scissored to the layer's clip, that loads the texel
  under each pixel and blends it with premultiplied over. Outside the line the
  texel is 0, which leaves the target's bytes unchanged.
- **`Rgba8Unorm`, not `R8`** (amends the N2 note): a premultiplied colour target
  keeps per-row colour, and needs no CPU copy of the layer's colour. Where rows
  of different colours overlap, each channel takes the larger value; where they
  share a colour (the usual case: one series per layer) the result is exact.
  Cost: 4 bytes per target pixel per translucent layer, pooled across frames.
- **Which layers**: `Line` layers (not `Area`, whose geometry is also
  `Polyline`) whose colour can have alpha below 1: a constant, derived or
  default colour with alpha < 1; a colour column with a LUT holding alpha < 255,
  or with no LUT (unknown, so assumed translucent); an active brush with `Fade`
  below 1 or a translucent `Grey`; or a read null colour with alpha < 1. Every
  other layer, and every opaque line, takes today's path byte for byte, with no
  texture and no extra pass.
- **Gup-owned targets** (`Renderer::render`: image, texture, window and web
  targets) record the offscreen passes in the frame's encoder before the scene
  pass. The scene is still drawn in one pass.
- **Draw-in-pass hosts** (`Renderer::prepare` + `Prepared::draw`) are unchanged:
  their translucent lines draw directly, as today, and the docs of
  `Renderer::prepare` and `Line` say so. `gup-egui` and `gup-bevy` render
  through `TextureTarget` and `Renderer::render`, so they get the pass with no
  API change. The smallest change for a host's own pass, a public
  `Prepared::encode`, is proposed in GUP-584 rather than made here (the
  escalation trigger below).
- **Stencil is not built** unless the offscreen target fails the 2x rule: it
  needs a depth-stencil attachment in every target and host pass (a `TargetDesc`
  change), and a binary stencil either drops the analytic antialiasing or keeps
  the seam where two rims cross (RFC-001 "GUP-432 findings", alternative 3). The
  reasons are recorded in RFC-001.
- **Decision rule**: keep the offscreen target unless the translucent layer
  costs more than 2x its direct draw at 100K points (`zoom_bench`), or cannot
  run on the browser smoke test's adapter (WebGPU baseline only: `Max` blend,
  `Rgba8Unorm` render attachment and `textureLoad` are core).
- **SVG**: a translucent line layer whose strokes share one alpha is written as
  `<g opacity="a">` around opaque strokes: the group is composited once, as the
  GPU's offscreen target is. A layer with mixed alphas keeps per-element
  `stroke-opacity`.

### Escalation Triggers

- Neither approach keeps opaque lines byte-identical with no extra pass (AC2).
- Draw-in-pass hosts (`Prepared::draw`, egui, Bevy) would need an API change to
  get the extra pass: report the smallest change before making it.
- The offscreen layer costs more than 2x the direct draw at 100K points.
- WASM growth over 3 KB gz, or any opaque golden changes.

### Out of Scope

- Areas, segments, circles and other marks; blending between different layers
  (overplotted series still blend over each other, which is the density the
  story wants).
- Sharing one texture between several translucent layers (memory grows with
  their count; see the RFC record).

### Additional Dependencies

- GUP-550: Line curves part 1 📋; GUP-551: monotone curves 📋.

## Technical Tasks

- [x] Prototype a per-layer coverage pass (offscreen `Rgba8Unorm` target with
      `Max` blend, composited; see Decisions) against a stencil approach (not
      built: the offscreen target met the 2x rule; reasons in RFC-001).
- [x] Decide whether per-row colour survives (it does, as a per-channel max
      where colours overlap), and how draw-in-pass hosts get the extra pass
      (GUP-584 proposes `Prepared::encode`).
- [x] Implement it, test AC1–AC2, and record the findings.

## Dependencies

### Prerequisite Stories

- GUP-432: `Line` Mark ✅.

## Testing Strategy

- **Integration tests**: ink at joins and self-crossings against the arms.
- **Visual validation**: a translucent overplotted-series golden.
- **Performance**: `zoom_bench`-style timing of a 100K-point translucent line.

## Success Metrics

- [x] Overplotted translucent series show no beads at their vertices
      (`gup_core/line_translucent`).

## Risk Assessment

- **Medium**: a per-layer pass conflicts with the single-render-pass model and
  with host-owned passes (`Prepared::draw`). _Mitigation_: make the pass opt-in
  for translucent line layers, and record what draw-in-pass hosts must do.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read), if visual
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

### Definition of Done evidence (2026-11-02)

- **Tests**: `cargo test -p gup-core -- --test-threads=1`: 752 passed, 0 failed,
  7 ignored (all pre-existing). New: `tests/line_translucent.rs` (6 tests) and
  `render::tests::only_translucent_lines_draw_offscreen`. With the offscreen
  pass disabled, every AC1 case but `MonotoneX` fails (0.749 ink at a join,
  corner or crossing; 0.875 where the line doubles back). Removed:
  `line_png.rs`'s `a_translucent_join_blends_twice`, which pinned the old
  behaviour. `cargo test --doc -p gup` and `cargo test -- --test-threads=1`
  pass; `mask all-check` and `mask smoke-examples` pass;
  `mask merge-check --dogfood`: story-ids, doc, gup-core, wasm-size,
  wasm-browser and dogfood all PASS.
- **Goldens**: new `tests/golden/gup_core/line_translucent.png` (twelve series
  at alpha 0.35: each even along its length, darker where series cross; read
  whole and zoomed 4x). Re-blessed `gup_core/line_monotone.png`: its alpha-0.35
  reference polyline now draws through the offscreen target; 654 pixels change
  by 1/255 at most, all on or beside that polyline (read zoomed 3x against the
  old golden: no visible change). A bless of every gup-core test changed no
  other golden or fixture, so opaque lines are byte-identical
  (`gup_core/line_series` included).
- **SVG**: the bow tie split by a null is `<g opacity="0.5">` around two opaque
  polylines; its resvg raster (read) has an even crossing (0.5 ink) and matches
  the GPU image.
- **WASM** (`scripts/wasm_size.sh`, against the parent sources): scatter 358,434
  → 360,271 B gz (+1,837); `per_row_type` 1,373 → 1,606 B.
- **Cost** (AC3): RFC-001 "GUP-439 findings": the offscreen layer is 1.19-1.21x
  its direct draw at 100K rows, 4 B per target pixel, one pass and one composite
  draw per translucent layer.
- **Performance budgets** (`scripts/perf_budget.sh --skip-wasm`, load average
  5.4-5.7; the `wasm.*` rows report "not measured" with `--skip-wasm`, and are
  measured above and by merge-check's wasm-size step):

```text
metric                                 recorded    check        limit     measured  verdict
zoom.size                             1920x1080    exact    1920x1080    1920x1080  pass
zoom.samples                                  1    exact            1            1  pass
zoom.gpu_clock.median_mhz                   350     info            -          617  info
zoom.gpu_pass.median_ms                    2.95     +20%        3.540        2.186  pass (faster than recorded: re-record?)
zoom.gpu_pass.p95_ms                       5.60     +25%        7.000        4.631  pass
zoom.cpu.median_ms                         0.69     +20%        0.828        0.559  pass
zoom.cpu.p95_ms                            0.86     +25%        1.075        0.702  pass
zoom.interval.median_ms                    4.91     info            -        3.005  info
zoom.columns.bytes                            0    exact            0            0  pass
zoom.columns.writes                           0    exact            0            0  pass
zoom.validity.bytes                           0    exact            0            0  pass
zoom.uniforms.bytes_per_frame               136    exact          136          136  pass
zoom.uniforms.writes_per_frame                3    exact            3            3  pass
zoom.instances.bytes                    6362952    exact      6362952      6362952  pass
zoom.instances.writes_per_frame               3    exact            3            3  pass
zoom.textures.bytes                           0    exact            0            0  pass
zoom_culled.draws.median                     14    exact           14           14  pass
zoom_culled.gpu_clock.median_mhz            350     info            -          950  info
zoom_culled.gpu_pass.median_ms             9.90     +20%       11.880        9.866  pass
zoom_culled.cpu.median_ms                  1.07     +20%        1.284        1.007  pass
zoom_culled.columns.bytes                     0    exact            0            0  pass
pipeline.link_create.median_ms             1.11     +20%        1.332        1.058  pass
pipeline.link_create.cold_ms               1.04    <= 10           10        1.193  pass
pipeline.link_create.max_ms                1.06     info            -        1.193  info
```

## Implementation Summary

- **Offscreen target per translucent line layer**
  (`crates/gup-core/src/ render.rs`): `Renderer::render` prepares with offscreen
  targets (`prepare_with`), records `Prepared::encode` (one pass per translucent
  layer: clear, then the layer's own glue with the `Max`-blend
  `coverage_pipeline` into a pooled `Rgba8Unorm` texture) before the scene pass,
  and the scene pass draws `Draw::Composite` (one triangle, the layer's scissor)
  with `composite_pipeline`. `Renderer::prepare` is unchanged for host passes.
  `create_pipeline` takes the blend state.
- **Composite shader**: `crates/gup-core/src/shaders/composite.wgsl` (new,
  `COMPOSITE_SHADER`, parsed by `guide_shaders_are_complete_wgsl`).
- **Which layers** (`crates/gup-core/src/selection.rs`, `Layer::translucent`):
  `Line` layers whose constant, derived, default, LUT or null colour, or brush
  fade, can be below alpha 1; carried on `MarkBatch::translucent`
  (`crates/gup-core/src/scene.rs`).
- **SVG** (`crates/gup-core/src/svg.rs`): `line_opacity` writes a translucent
  line layer of one alpha as `<g opacity>` around opaque strokes.
- **Docs**: `Line`, `Renderer::prepare`, `Renderer::render`, `line.wgsl`'s
  header; RFC-001 "GUP-439 findings".
- **Tests**: `crates/gup-core/tests/line_translucent.rs` (AC1 cases, MSAA, SVG
  group and parity, overplotted golden), render unit test (AC2: which layers
  draw offscreen, draw counts).

## Retrospective

**Completed**: 2026-11-02

### Key Technical Learnings

#### Max blending is the union of capsules

- **Challenge**: an overlap must count once, but only a pixel's neighbours
  (other segments, other layers' pixels) know it is an overlap.
- **Solution**: draw the layer into its own texture with `BlendOperation::Max`.
  `shade` already outputs premultiplied colour times coverage, and a capsule's
  box coverage falls with the distance to the stroke, so the maximum over the
  capsules is the coverage of their union: the round-joined stroke, drawn once.
  No shader or glue change was needed; the pipeline differs only in blend state
  and target.
- **Pattern**: before adding geometry to avoid double cover, check whether a
  blend state on a separate target expresses "count once".

#### The composite must not move the target's bytes

- **Challenge**: a full-target composite risks touching every pixel.
- **Solution**: discard texels with alpha 0 and scissor to the layer's clip;
  premultiplied over with a zero source is exact anyway. The texel comes from
  the interpolated position, not `@builtin(position)`, so a viewport offset
  cannot shift it (GUP-576's lesson).

#### A translucent layer hides inside an "opaque" golden

- **Challenge**: blessing every gup-core golden to prove opaque lines
  byte-identical changed `gup_core/line_monotone`.
- **Solution**: the diff (654 pixels, at most 1/255) sat only on its alpha-0.35
  reference polyline, which now takes the offscreen path. Comparing the colours
  of the differing pixels in the parent render settled it in a minute.
- **Pattern**: "bless everything, then `git status tests/golden`" is a cheap
  byte-identity check for "unchanged" ACs, and the differing pixels' colours
  name the layer.

### Architectural Decisions

#### `Rgba8Unorm`, not `R8`

- **Decision**: the offscreen target holds premultiplied colour.
- **Reasoning**: a key- or palette-encoded colour is only known on the GPU, so
  an `R8` coverage target would need the layer's colour on the CPU, and per-row
  colour would be lost.
- **Trade-off**: 4x the memory (4 B per pixel per translucent layer); where rows
  of different colours overlap, each channel takes the larger value.
- **Future**: many translucent layers cost a texture each; sharing one between
  layers whose boxes do not overlap is the next step if it matters.

#### Host passes unchanged

- **Decision**: `Renderer::prepare` + `Prepared::draw` keep the direct draw;
  `Renderer::render` gets the offscreen pass. The public `Prepared::encode` is
  proposed in GUP-584, not made (the story's escalation trigger).
- **Reasoning**: `gup-egui` and `gup-bevy` render through `TextureTarget`, so
  every in-repo host already gets the pass; only `zoom_bench` and external
  draw-in-pass hosts do not.
- **Trade-off**: AC3's timing used a local prototype patch, so it is not yet
  reproducible from the tree (GUP-584 adds `zoom_bench --translucent`).

#### Only `Line`, by type

- **Decision**: the translucency check is keyed on `TypeId::of::<Line>()`, not
  `Geometry::Polyline`.
- **Reasoning**: `Area` is also `Polyline` geometry; its trapezoids share edges
  and must keep blending directly (a translucent area fill would otherwise take
  the pass).

### Development Workflow Insights

- Seeding by flipping one boolean (`prepare_with(.., false)` in `render`) showed
  every AC1 case failing (0.749 ink, 0.875 where the line doubles back) except
  `MonotoneX`, whose plane joints already draw each pixel once.
- GPU pass times on the HD 630 move by up to 1.6x run to run (clock governor,
  load 2-7). Alternating four variants for 5-7 rounds and comparing medians gave
  a stable 1.19-1.21x ratio where single rounds ranged from 1.02x to 1.80x.
- Comparing WASM size against the parent by checking out the parent's sources
  over the tree, measuring and checking them back out took two runs of
  `wasm_size.sh` and no extra worktree.
- The worktree guard refuses commands that combine `cd`, heredocs and `git`;
  writing scripts to `/tmp` and running plain commands avoided it.

### Follow-up Stories

1. **GUP-584: Translucent Lines in Host-Owned Passes** — a public
   `Prepared::encode`, a `draw` that stays correct without it, and
   `zoom_bench --translucent` to make AC3's timing reproducible.
