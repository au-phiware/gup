---
implementer: sonnet
implementer-reason:
  two small task binaries ported with their code and checks fixed in Decisions;
  follows GUP-465's port and region rules
---

# GUP-485: RFC-001 Step S14a: Last Dogfood Tasks on `gup-core`

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-23)
**Created**: 2026-10-19

## Context

The S14 flip deletes the old render path, so nothing outside it may still use
it. After GUP-465 and GUP-480, three dogfood binaries remain on the old path:
`ref_export_png` (a raw old `Selection` with `CircleInstance`s inside a
`ComposedChart`), `t6_wgsl` (the old `#[wgsl_function]` through `attr_shader`,
also run as `t6_wgsl_tutorial` with `RAW_MACRO=1`), and `t5_stream`, which
GUP-480 ports to the appender.

Their tracked gaps are old-path bugs the new core has fixed: `NO_TEXT`,
`DOUBLE_GAMMA`, `NO_GRID`, `SELECTION_CLIP_SPACE` and `WGSL_DUPLICATE_UNIFORMS`.
Porting the two binaries turns every one of those checks into a plain check, so
the dogfood suite has no tracked gap left from the old path, and
`dogfood/Cargo.toml` can drop its `gup` dependency until the flip re-adds it
(GUP-488).

## Read First

- [GUP-465](GUP-465_RFC_001_S10f_Dogfood_On_Gup_Core.md) "Acceptance Criteria"
  AC2 (the bar does not move; regions from `Layout`) and "Decisions".
- `dogfood/src/bin/ref_export_png.rs` (whole) and `dogfood/src/bin/t6_wgsl.rs`
  (whole).
- `dogfood/src/suite.rs`: gaps (lines 283–305), `fn ref_export_png` (lines
  427–500), `fn t6_tutorial` and `fn t6_workaround` (lines 835–906), and
  `title_band` (line 315).
- `crates/gup-core/tests/wgsl_function.rs` lines 14–60 and 395–405
  (`#[wgsl_function]` v2 with `Params`, f32 and `vec4<f32>` outputs, a user
  function on `Circle::RADIUS`).
- `crates/gup-core/src/target.rs` `ImageTarget::with_options` (line 357).
- `dogfood/README.md` "Control and reference" and "Task 6".

## User Story

> "As the project, I want every dogfood task on the new core, so that the flip
> can delete the old path with the external suite still green."

## Acceptance Criteria

### AC1: `ref_export_png`

- [x] Rewritten as in Decisions; imports only `gup_core` (and non-Gup crates).
- [x] Its checks keep their measures and thresholds. The gap checks "steel blue
      renders as configured", "title text present", "grid lines drawn in the
      empty plot area" and "no points left of the y axis" become plain checks;
      regions that moved are re-derived from a fresh render's `Layout` and
      listed old → new in the Implementation Summary.

### AC2: `t6_wgsl`

- [x] Rewritten as in Decisions, with no workaround type and no `RAW_MACRO`
      branch; imports only `gup_core`.
- [x] `t6_tutorial` and `t6_workaround` become one task, `t6_wgsl`, with the
      workaround task's checks: "fill renders as configured" becomes a plain
      check; the column regions are re-derived from `Layout` (the plot now has
      axes) and listed old → new. No threshold is lowered.

### AC3: Gaps and dependency

- [x] `NO_TEXT`, `DOUBLE_GAMMA`, `NO_GRID`, `SELECTION_CLIP_SPACE` and
      `WGSL_DUPLICATE_UNIFORMS` are deleted from `suite.rs`; `git grep` finds no
      use. If GUP-480 has landed, `STREAM_NEVER_READY` is already gone.
- [x] `dogfood/Cargo.toml` no longer depends on `gup` (nor `tokio`, if nothing
      else uses it). `git grep -n "gup::" dogfood/` is empty.
- [x] `xvfb-run -a ./dogfood/run_all.sh` reports no FAIL, no XPASS and no XFAIL:
      paste the summary line.
- [x] The implementer reads `ref_export_png.png` (title, ticks, grid, ten
      steel-blue points with dark rims inside the plot) and `t6_wgsl.png` (a
      20×20 grid, radii growing left to right, blue to orange-red).

### AC4: Docs

- [x] `dogfood/README.md` describes both tasks as they now are (no tracked
      failures, the dropped subtitle), and its intro says no task uses the old
      path.

## Decisions

- **`ref_export_png`**: the same ten `sample_data()` points;
  `let mut plot = Plot::new();` with `plot.x(Linear::new().domain(0.0, 11.0))`,
  `plot.y(Linear::new().domain(30.0, 100.0))`,
  `.title("Study Hours vs Test Score")`, `.grid(Axes::Both)`; one
  `circles(data)` layer with `.x(|d| d.x)`, `.y(|d| d.y)`, radius `Px(8.0)` (the
  old 0.02 clip-space radius at 800 px), fill `Color::rgb(0.22, 0.46, 0.82)`,
  stroke `Color::hex(0x1a1a1a)` 2 px. Outputs: `save_png` 800×600; the `@2x`
  file through
  `ImageTarget::with_options(&cx, 1600, 1200, TargetOptions { dpr: 2.0, ..Default::default() })`
  rendering the scene of `plot.resolve(&cx, 800.0, 600.0)`; the large file by
  `save_png` 2400×1600. The subtitle is dropped (gup-core has no subtitle) and
  the README says so. The gallery screenshot hook is dropped.
- **`t6_wgsl`**: the same 400 readings (`x`, `y` in −0.9..0.9, `v = gx`) as
  `f64`. Two user functions:

  ```rust
  #[derive(Clone, Debug)]
  struct Knee { min_r: f32, max_r: f32 }
  #[wgsl_function(wgsl = r#"
      struct Params { min_r: f32, max_r: f32 }
      fn knee_radius(v: f32, p: Params) -> f32 {
          return p.min_r + (p.max_r - p.min_r) * (v * v) / (v * v + 0.1);
      }
  "#)]
  impl ShaderFn for Knee { fn params(&self) -> KneeParams { .. } }

  #[derive(Clone, Debug)]
  struct Ramp;
  #[wgsl_function(wgsl = r#"
      struct Params { pad: f32 }
      fn ramp(v: f32, p: Params) -> vec4<f32> {
          return vec4<f32>(v, 0.3, 1.0 - v, 1.0);
      }
  "#)]
  impl ShaderFn for Ramp { fn params(&self) -> RampParams { RampParams { pad: 0.0 } } }
  ```

  on a `Plot` with fixed domains −1..1 on both axes, a `Circle` layer with
  `RADIUS` = `Knee { min_r: 1.5, max_r: 12.0 }.encode(|r| r.v)` (the old
  0.005–0.04 clip-space radii at 600 px) and `FILL` = `Ramp.encode(|r| r.v)`,
  saved as `t6_wgsl.png` at 600×600. Pad `Params` as the macro requires (it
  reports the rule at compile time). No identifier ends in a digit.

- **Unchanged**: every other task.

## Escalation Triggers

- A check fails on the port and passing it would need a lowered threshold or a
  gup-core change.
- `#[wgsl_function]` rejects either function as written for a reason other than
  `Params` padding.
- Any check XPASSes or turns XFAIL in a task this story does not touch.

## Out of Scope

- `t5_stream`: GUP-480.
- Switching dogfood imports to `gup::`: GUP-488 (the flip).

## Technical Tasks

- [x] Port `ref_export_png`; re-derive regions; turn gaps into checks.
- [x] Port `t6_wgsl`; merge the two tasks; re-derive regions.
- [x] Delete gaps and the `gup` dependency; README; suite run.

## Dependencies

### Prerequisite Stories

- GUP-465: dogfood on gup-core ✅.
- GUP-480: RFC-001 S12a Appender Handle 📋 — ports `t5_stream`, the last other
  `gup` user, so AC3 can drop the dependency.

### Enables Stories

- GUP-486: S14b — no dogfood task uses `ComposedChart`.

## Testing Strategy

- **End to end**: `xvfb-run -a ./dogfood/run_all.sh`;
  `cargo test --manifest-path dogfood/Cargo.toml`.
- **Visual validation**: the two PNGs in AC3.

## Success Metrics

- [x] Dogfood has no XFAIL and no dependency on the old path.

## Risk Assessment

- **Low**: the colour check "fill renders as configured" (tol 16, 500 px in the
  v=1 column) assumes `vec4(1, 0.3, 0, 1)` renders as `#ff4d00`; gup-core's
  colour policy treats shader colours as sRGB-encoded, so it should.
  _Mitigation_: escalation trigger above.
- **Low**: trybuild `.stderr` churn: none (no gup-core edits).

## Definition of Done

- [x] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [x] All Acceptance Criteria are satisfied and checked
- [x] `cargo test --manifest-path dogfood/Cargo.toml` passes
- [x] `xvfb-run -a ./dogfood/run_all.sh` passes (no FAIL, no XPASS, no XFAIL)
- [x] Lint and format clean: `mask all-fix`
- [x] `mask wasm-browser`: not needed (dogfood only; no layout, input or wasm
      change); say so in the Implementation Summary
- [x] WASM size: no library code changes; record "unchanged"
- [x] Rendered output verified by eye: `ref_export_png.png`, `t6_wgsl.png`
- [x] `mask old-path-loc` recorded (expected unchanged)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `ref_export_png` and `t6_wgsl` ported to `gup_core` as in Decisions. `Knee`
  needed `type Out = Px;` (a `Circle::RADIUS` channel is `Px`; the macro's own
  rule, not an escalation). The subtitle and the gallery hook are dropped.
- Suite: `NO_TEXT`, `DOUBLE_GAMMA`, `NO_GRID`, `SELECTION_CLIP_SPACE`,
  `WGSL_DUPLICATE_UNIFORMS`, the unused `gap()` helper and the
  `t6_wgsl_tutorial` task are deleted; `t6_wgsl_workaround` is now `t6_wgsl`.
  The `Gap` / `Exit::Fails` runner machinery stays for future gaps.
- Regions, old -> new (from fresh `Layout`s; thresholds unchanged):
  - ref "no points left of the y axis": (0, 0.05, 0.15, 0.95) -> (0, 0.07,
    0.045, 0.945); plot rect is x 0.051..0.985, y 0.07..0.945.
  - t6 v=0 column: (0, 0, 0.08, 1) -> (0.10, 0.02, 0.15, 0.945); v=1 column
    (0.92, 0, 1, 1) -> (0.92, 0.02, 0.97, 0.945); plot rect x 0.08..0.98, y
    0.02..0.945.
- `dogfood/Cargo.toml` drops `gup` and `tokio`; `git grep "gup::" dogfood/src`
  is empty. README updated.
- Gate: `mask all-fix`/`all-check` now run
  `cargo clippy --manifest-path dogfood/Cargo.toml --all-targets -- -D warnings`;
  fixed `ChartTexture` (`Default`) and a collapsible `if` in `dogfood_check`.
- Evidence: dogfood suite `Tasks: 10 pass, 0 XFAIL, 0 FAIL, 0 XPASS` (Xvfb +
  lavapipe); `cargo test --manifest-path dogfood/Cargo.toml` 18 pass;
  `mask all-check` passes; old-path LOC 26,601 (unchanged); WASM over_wgpu
  299,241 B gz (unchanged, no library change); `mask wasm-browser` not needed.
  Read `ref_export_png.png` (title, ticks, grid, ten steel-blue points with dark
  rims inside the plot) and `t6_wgsl.png` (20x20 grid, radii growing left to
  right, blue to orange-red).

## Retrospective

**Completed**: 2026-10-23

### Key Technical Learnings

- **Challenge**: `Knee` rejected on `Circle::RADIUS` (Output f32 vs Px).
  **Solution**: `type Out = Px;` in the `ShaderFn` impl. **Pattern**: pixel
  channels need `Out = Px`.
- The `t3_scatter_window` run failed with a Vulkan ICD from the i686 lavapipe
  json; use `lvp_icd.x86_64.json`.

### Architectural Decisions

- Kept the `Gap` machinery with no current user, so the next tracked gap needs
  only a helper, not a framework. Trade-off: unused-code surface in the runner.

### Development Workflow Insights

- A temporary bin printing `resolve(..).layout.plot` gave exact regions in one
  run.

### Follow-up Stories

None.
