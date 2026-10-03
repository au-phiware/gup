# Strategic Review — October 2026

**Status**: Proposal, awaiting owner decisions (see
[Decisions](#decisions-needed)) **Date**: 2026-10-04

## Summary

Gup has ~340 completed stories, 193k LOC in `src/`, 110 examples and 4,600+
passing tests. Most of the individual components work in isolation. **They do
not yet add up to a product.** Five independent audits — retrospective trends,
architecture, public API, visual quality and real-world dogfooding — reached the
same conclusion from different directions:

- **The core idea is not the production path.** The unified shader-function
  system (TECHNICAL_APPROACH) has zero production callers: `attr_shader` is only
  used in `selection.rs` tests. Chart builders compute positions on the CPU in
  closures, look up attributes by string, and hand finished instances to
  hand-written per-mark shaders.
- **Output is broken by default.** PNG/texture output has no text (no titles,
  tick labels or legends). `render_to_svg` passes an empty mark slice. Colours
  are double gamma-encoded (`#1f77b4` → `rgb(142,189,219)`). Circles are
  ellipses on non-square canvases. About half the examples are blank, wrong or
  panic. All four `composite_*` examples panic on the first frame, including the
  one added by GUP-365 two days before the audit.
- **The API is hard to use.** It has 1,235 public items and ~600 names at the
  crate root via 16 glob re-exports. There are two GPU context types that can't
  work together, so a builder chart can't be shown in a window. Some methods are
  placeholders (`AccessorFunction::from_field` → `0.0`; `ConfiguredChart::build`
  → always `Err`) and several are silent no-ops. `hello_world.rs` is 265 lines
  of raw wgpu.
- **It is not adoptable.** The dogfood audit completed 3 of 6 realistic tasks,
  and only with ~50% workaround code. `gup-egui` doesn't compile against HEAD.
  `#[wgsl_function]` in an external crate panicked at runtime.
- **About 40k LOC is unwired** (GPU culling/LOD, Mixable, the Rust→WGSL
  transpiler, duplicate scale and pipeline-cache systems, error recovery). About
  17k LOC of debug and benchmark code lives in the core crate.

The GPU engineering underneath is genuinely good: 200k points build in 38 ms and
export in 45 ms in release. The problem is integration, not capability.

## Root cause: the process optimised for local correctness

The retrospective analysis explains how this happened:

| Symptom                                                  | Evidence                                                                                                                                                   |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Stories scoped to one instance, not the class of problem | Median story ~26 min. 48 stories are ≥4 levels deep in follow-up chains. The "builder never calls `prepare_render_bound`" bug took six sequential stories. |
| Backlog growing, not converging                          | 75–85% of stories are retro follow-ups (~1.35 per retro). March: 149 created vs 101 completed.                                                             |
| ACs ticked while output was broken                       | Charts with 0 visible pixels passed every checked AC. GUP-379's test only checks for "non-white pixels", so the area-chart fan explosion passed.           |
| Quality gate routinely bypassed                          | `--no-verify` appears in 63 retros (pre-commit hook failed on lint debt and scanned other worktrees). **Fixed 2026-10-04**: the hook passes on `main`.     |
| No end-to-end or user-perspective validation             | Nobody ran the examples, looked at the PNGs, or used the crate externally.                                                                                 |
| Metadata drift                                           | 34 duplicate story IDs. Agents invent dates ("Completed 2025-07" on 2026-03 work).                                                                         |
| Build footprint                                          | ~120 integration-test binaries (20–60 MB each). Disk exhaustion in 28% of recent retros.                                                                   |

**Lesson:** agents verify exactly what the story asks them to verify. If no
story asks "does a user get a correct, good-looking chart?", nobody checks. From
here on, end-to-end, pixel-level and external-crate validation must be part of
the definition of done, not a follow-up.

## North star

> A user writes ~5–10 lines and gets a publication-quality chart, identical in a
> window, PNG, SVG and WASM, with data encodings evaluated on the GPU, and can
> drop from that chart into the low-level `Selection` API without starting over.

```rust
let chart = gup::scatter(&data)
    .x(|d: &Row| d.gdp).y(|d: &Row| d.life_exp)
    .color(|d: &Row| d.continent.as_str())
    .size(|d: &Row| d.population);
chart.save_png("out.png", 800, 500)?;   // or gup::show(chart)
```

## Strategy: tracks and sequencing

Order matters. Prune first so later work migrates less code. Design the core
before redesigning builders on top of it. Visual bug fixes that will survive the
redesign can start immediately.

```text
T0 Guardrails ──────────────────────────────────────────────▶ (continuous)
T1 Prune & public surface ──▶
T4a Visual quick wins ──────▶
T3 Core RFC + spike ────────▶ T2 One Context / Scene / RenderTarget ──▶
                                   T3b Core data model ──▶ T5 Builders on core ──▶ T6 Flagship gallery
                                                                                  T7 Crate split, LOD, integrations
```

### T0 — Guardrails (start now, never finish)

1. **Visual regression harness**: golden-image tests for every chart builder
   (including titles, tick labels, legend). They fail on blank output, missing
   text or geometry outside the plot. Replace per-builder "non-white pixel"
   tests.
2. **Examples smoke test**: every example builds _and_ runs headless for N
   frames without panicking (`GUP_SCREENSHOT_PATH` already exists).
3. **Dogfood suite in CI**: promote the audit's external crate (6 realistic
   tasks; source saved at `/tmp/gup-dogfood-src`) to `dogfood/`, run by CI.
   Integration crates (`gup-egui`, `gup-bevy`, …) must compile in CI.
4. **Process changes** to `story-worker.md` / `story-scribe.md` (the
   `.claude/agents` and `.github/agents` copies are identical duplicates and
   should become one file plus a symlink):
   - Fix the _class_ of problem: audit all siblings before closing a story.
   - Definition of done includes "renders correctly" evidence (a golden image or
     attached PNG the agent has actually looked at) for anything visual.
   - Never `--no-verify`. If the hook fails for unrelated reasons, stop and
     report.
   - Dates come from `date -I` / git, never invented.
   - Before writing a follow-up, check the backlog and code for existing
     coverage. Allocate IDs via a script (GUP-383).
   - Stop adding entries to per-story learnings in agent docs without pruning.
     Fold recurring retro themes in monthly.
5. **Build footprint**: consolidate `tests/*.rs` into a few harness binaries.
   Set a default `CARGO_TARGET_DIR` in the dev shell.

### T1 — Prune and public surface (S–M, low risk)

- Delete or quarantine: transpiler + `shader_ast` + `#[shader_fn]` (~18.7k);
  Mixable / async_mixable / integration / plugins / `RenderLayerManager`
  (~8.4k); `MarkRenderer` and friends (~6.4k); `plot_api`, `FieldAccessor`,
  `BoundChartBuilder`, `examples` module; duplicate scale systems and pipeline
  caches (~3.2k); unused error-recovery modules (~3.5k).
- Move debug, benchmark and performance-report code behind a `debug` feature or
  into `gup-debug` (~17k).
- Replace the 16 root glob re-exports with a curated prelude of ~25 names. Make
  internals `pub(crate)`. Fix duplicate names (`AccessorFunction`, `Margins`,
  `Color`, 4× `Orientation`, …).
- Delete silent no-op builder methods rather than leaving them as stubs.

### T2 — One Context, one Scene, one RenderTarget (M, medium risk)

- Merge `RenderContext` and `GupContext` into `gup::Context` (`new_blocking()`,
  `from_wgpu()`). Re-export wgpu.
- A resolved chart **Scene** (marks, axes, ticks, labels, title, legend, grid,
  clip rect) consumed by a `RenderTarget` trait: surface, texture (egui/bevy),
  PNG readback, WASM canvas. SVG and PDF walk the same scene.
- `GupApp` / `gup::show()` render any chart. Rewrite `hello_world` in ~10 lines.
- One object-safe `Chart` trait replaces `Mixable`, `ComposedChart`,
  `CompositeChart` and egui's `DynChart`.

### T3 — Core data model (L, high risk; RFC and spike first)

- **Typed channels**: `Circle::RADIUS: Channel<Circle, f32>`. `attr` accepts a
  CPU closure _or_ a `ShaderFn` whose output type is checked by trait bounds.
  Unknown or mistyped attributes become compile errors, not silent drops.
- **GPU columns**: accessor outputs are uploaded once as structure-of-arrays.
  Rescaling or zooming changes uniforms, not data.
- **Scales as shader functions with a CPU mirror** for axes and ticks. This
  collapses the four scale systems into one family: Linear, Log, Band, Time,
  Color.
- **Layout object**: domain, plot rect and band positions resolved in one place
  and exposed publicly (`chart.layout().invert(px)`, `chart.pick(px)`).
- **Units**: `Px` / `Ndc` newtypes. No ambiguous bare-`f32` sizes.
- **Scale-ready storage**: chunked columns (each chunk within binding limits),
  per-chunk origin offsets for f32 precision, async readback only. This removes
  the structural blockers to the billion-point goal.
- Composition by module (naga_oil-style), not WGSL text splicing.

### T4 — Visual design system

- **T4a quick wins** (independent of T3, survive the redesign):
  - Linearise colours for sRGB targets.
  - Pixel-space, aspect-correct circle radius.
  - MSAA or analytic SDF anti-aliasing.
  - Replace the Squada One default font with a neutral UI face with tabular
    figures.
  - Derive tick precision from the tick step.
  - Fix the area-chart stroke width (GUP-384).
  - Fix the `composite_*` first-frame panic.
- **T4b**:
  - A `Theme` type (light/dark) with one modern categorical palette and
    viridis/cividis sequential defaults.
  - Quad-expanded lines with real widths, joins and caps.
  - Margins computed from measured label sizes.
  - Axis titles, left-aligned title and subtitle.
  - Band scales for categorical axes.
  - Clip marks to the plot rect.

### T5 — Builders on the new core, plus real-world features (L)

Port the builders (~17k LOC) to channels, scales and the scene. Keep the user's
`T` in output types. Add `chart.layer_mut::<M>() -> &mut Selection<T, M>`.
Deliver what the dogfood audit found missing:

- time scales with calendar ticks;
- legends;
- log decade ticks;
- band-axis labels and value labels;
- null handling;
- `append()` with tail-only upload;
- built-in tooltips and picking;
- real grouped and stacked bars.

### T6 — Flagship gallery and docs-as-tests (M)

Replace the debug-demo gallery with 8–10 polished, CI-rendered charts, including
one that shows the GPU advantage (1M-point density scatter). Add a README hero
image. Every tutorial snippet compiles and renders under test.

### T7 — Later

- Split the workspace (gup-core, gup-text, gup-charts, gup-interact, gup-a11y,
  gup-export, gup-geo, gup-debug). Leaf crates can split during T1.
- Feature-gate platform dependencies (zbus, tokio, image, chrono, csv).
- Wire GPU culling and LOD into core.
- Re-wire mobile and framework integrations onto the single render path.

## Backlog triage

Freeze stories that extend the CPU path or the APIs scheduled for replacement.
They stay in the index marked ⏸ Parked and are re-evaluated after T5.

| Action                                  | Stories                                                                                                                                                                                                                                 |
| --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Park** (superseded by T2/T3/T5)       | GUP-366–369 (choropleth), GUP-285 Legend, GUP-286 Per-bar fill, GUP-295, GUP-300, GUP-361, GUP-385, GUP-307, GUP-287, GUP-268B/C, GUP-272/273/353 (mobile chart wiring), GUP-262C/D/E, GUP-263C/D, GUP-269C/D, GUP-355/356/357, GUP-360 |
| **Fold into T4a**                       | GUP-384 (area stroke width)                                                                                                                                                                                                             |
| **Fold into T0**                        | GUP-383 (ID guard), GUP-374 (renumbering; refresh its ID table first, since 384–387 are now taken), GUP-285B (WASM test compile)                                                                                                        |
| **Close: done by the 2026-10-04 merge** | GUP-387 (markdown lint debt)                                                                                                                                                                                                            |
| **Rescope**                             | GUP-370 → "instrument treemap with existing `GpuTimer` (GUP-292)"                                                                                                                                                                       |
| **Keep**                                | GUP-386, GUP-375, GUP-376 (small, survive the redesign), GUP-312–315 / 371–373 (re-evaluate after T3 RFC)                                                                                                                               |

## Decisions needed

1. **Prune**: approve deleting ~40k LOC of unwired subsystems? Alternative: move
   the transpiler and GPU culling/LOD into an `experimental/` crate rather than
   deleting them.
2. **Breaking API reset**: approve breaking changes with no deprecation period
   (pre-alpha, no external users known)?
3. **Integrations during the rebuild**: keep `gup-egui`, `gup-bevy`, `gup-ios`,
   `gup-android` and `gup-tauri` compiling throughout (slower), or exclude them
   from the workspace until T2 lands (faster, then re-wire)?
4. **Design defaults**: default font (Inter vs Source Sans 3) and categorical
   palette (Tableau 10 vs Okabe-Ito, which is colour-blind safe).
5. **Process changes**: approve the T0 edits to the story-worker and
   story-scribe instructions.
6. **Docs**: once approved, `IMPLEMENTATION_STRATEGY.md` and
   `TECHNICAL_APPROACH.md` should be revised so they describe the architecture
   that actually exists or is being built, not an aspirational one.

## Proposed first wave (after approval)

Run in parallel in isolated worktrees, merged in order:

1. **T0 guardrails**: visual regression harness + examples smoke test + agent
   instruction changes (story-worker).
2. **T1 prune and surface purge** (story-worker; story-scribe writes the
   stories).
3. **T3 core RFC + scatter spike** (Plan agent → RFC doc for review before any
   port begins).
4. **T4a visual quick wins** (story-worker).

## Appendix: audit sources

All five audits ran 2026-10-04 as independent subagents. Renders are in
`/tmp/gup-visual-audit/` and dogfood outputs in `/tmp/gup-dogfood/` (both
ephemeral). Key verified evidence:

- `attr_shader` callers outside `src/selection.rs`: 0.
- `src/chart_builder.rs:2645` — `self.export_svg_with_marks(options, &[])`.
- `src/chart_builder/builders/density.rs:983` — `let _kde_result = …` (computed
  and discarded).
- `src/chart_builder/builders.rs:530` — `from_field` returns
  `AccessorValue::Float(0.0)`.
- `src/chart_builder/plot_api.rs:373` — `ConfiguredChart::build` always returns
  `Err`.
- `src/render.rs:13` `RenderContext` and `src/context.rs:1155` `GupContext`: two
  device-owning contexts.
