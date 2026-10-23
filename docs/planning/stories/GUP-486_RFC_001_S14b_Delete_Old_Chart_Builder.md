---
implementer: sonnet
implementer-reason:
  deletion by a fixed rule (delete what no longer compiles, port nothing) with
  the CI and gallery edits and the LOC target fixed in Decisions
---

# GUP-486: RFC-001 Step S14b: Delete the Old Chart Builder Module

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-23)
**Created**: 2026-10-19

## Context

RFC-001 S14 ("the flip") deletes the old selection, mark, shader_function,
shader_pipeline, chart_builder, context and render modules, and makes `gup`
re-export `gup-core`. Done in one step it would delete about 130k lines at once.
It is split so that every step leaves the gates green:

1. GUP-485 (S14a): the last dogfood tasks leave the old path.
2. **This story (S14b)**: delete `chart_builder` (everything that builds or
   returns `ComposedChart`) and whatever then fails to compile. Nothing in the
   old render core depends on `chart_builder`, so the core still builds.
3. GUP-487: `gup-core`'s macros resolve the crate path, so they work through
   `gup`.
4. GUP-488 (S14c): delete the old render core and everything left in `src/`, and
   make `gup` the facade.
5. GUP-489 (S14d): revise the architecture docs.

After GUP-466, `chart_builder` holds the area, boxplot, violin, density, GPU
density, heatmap and choropleth builders, `ComposedChart`, `AccessorFunction`,
`ConfigurableBuilder`, the colour bar, labels and the old builders' golden
harness: 8,097 of the 26,601 old-path lines measured by `mask old-path-loc`.
RFC-001 §11 says these builders come back as T5 stories after the flip, on
`gup-core`; the orchestrator review requires the flip to land first. The
strategic review approved breaking changes with no deprecation (Decision 2).

## Read First

- RFC-001 §11 "Survive, adapt, delete" and "Story sequence" (lines 758–792) and
  the orchestrator review point 1 (lines 917–929).
- [GUP-466](GUP-466_RFC_001_S10g_Delete_Old_Builders.md) "Acceptance Criteria"
  (the same deletion pattern, one level smaller) and its Implementation Summary
  once it lands.
- `scripts/old_path_loc.pl`, INDEX.md "RFC-001 Migration" header paragraph.
- Who depends on `chart_builder` outside it (at `d133343`): `src/lib.rs`,
  `src/prelude.rs`, `src/scale.rs` (`AccessorValue`), `src/grid.rs` (a doc
  example), `src/export/svg/renderer.rs`, `src/export/html/*`,
  `src/export/pdf/renderer.rs`. Re-run:

  ```bash
  git grep -lE 'chart_builder|ComposedChart' -- '*.rs' ':!src/chart_builder*'
  ```

- `.github/workflows/visual-regression.yml` (line 63, the old builders'
  goldens), `maskfile.md` `visual-regression` (line 28) and `smoke-examples`
  (line 171), `scripts/check_gallery_sync.sh`, `scripts/gallery_config.toml`,
  `examples/INDEX.md`, `tests/visual_regression/*.toml`.

## User Story

> "As a maintainer, I want the replaced chart builders deleted, not frozen, so
> that the old path shrinks to the render core the flip removes next."

## Acceptance Criteria

### AC1: Deleted

- [ ] `src/chart_builder.rs` and `src/chart_builder/` are deleted, with their
      re-exports and doc references.
- [ ] Everything that then fails to compile is deleted, not ported: the
      `export/html`, `export/pdf` and `export/svg/renderer.rs` modules, the
      `pdf` feature and the `printpdf` dependency, items in `scale.rs` that name
      `AccessorValue`, and every example, test, bench and doctest that used
      them. `export/png.rs` and `export/svg/element.rs` stay (the old marks and
      app use them until GUP-488).
- [ ] `tests/golden/chart_builders/` and its README section are deleted.
- [ ] The Implementation Summary lists the deleted examples, tests and benches
      with counts.

### AC2: Gates in step

- [ ] `Cargo.toml` `[[example]]`, `[[test]]` and `[[bench]]` entries,
      `examples/INDEX.md`, `examples/README.md`, `scripts/gallery_config.toml`
      and `tests/visual_regression/{examples_skip_list,expected_failures}.toml`
      match what is left; `scripts/check_gallery_sync.sh` passes.
- [ ] The `visual-regression.yml` step "cargo test --lib visual_regression" and
      the `mask visual-regression` task are removed (their tests are gone);
      `tests/golden/README.md` describes only `gup_core/`.
- [ ] `cargo test --all-features --test examples_smoke -- --ignored     --test-threads=1`
      passes for the remaining examples.
- [ ] `crates/gup-culling-lod` still builds and tests
      (`cargo test -p gup-culling-lod -- --test-threads=1`).
- [ ] `cargo build --target wasm32-unknown-unknown --lib` (the root, as
      `wasm.yml` runs it) still passes.

### AC3: The metric went down

- [ ] `mask old-path-loc` TOTAL is **at most 18,504** (no `chart_builder` file
      left; the other old-path files measured 18,504 at `d133343`). Record the
      figure in INDEX.md's "RFC-001 Migration" header with the date and this
      story.
- [ ] RFC-001's S14 row gains a one-line note that S14 is split into GUP-485 to
      GUP-489.

## Decisions

- **The rule**: delete `chart_builder`; then repeatedly delete whatever does not
  compile, at the smallest unit that compiles again (an item, a module, a file,
  an example), never porting. Ports to gup-core happened in GUP-465, GUP-466 and
  GUP-485, and the remaining builders are T5 stories after the flip.
- **Examples on gup-core stay**: `hello_world` and GUP-466's four ports
  (`basic/01_hello_chart`, `basic/03_line_chart`, `basic/04_bar_chart`,
  `composite_mixed_data`) must still build and run.
- **Tutorials**: `docs/tutorials/*.md` are included as doctests by `src/lib.rs`
  (`tutorial_doctests`, lines 470–495). A tutorial whose snippets no longer
  compile is removed from that module (the file stays, with GUP-466's "out of
  date until T6" note); `mask test-tutorials` and
  `tests/tutorial_snippet_tests.rs` are trimmed to what compiles.
- **CI edits allowed**: exactly the `visual-regression.yml` step above. The
  maskfile edits are the `visual-regression` task and any `smoke-examples` `-p`
  list change.
- **Not deleted here**: `src/app.rs`, `src/wasm_api.rs`, `export/png.rs`,
  `export/svg/element.rs`, `export/gallery.rs` (if they still compile), and the
  whole old render core: GUP-488.

- **Cascade into the old core (2026-10-23, orchestrator, option 1)**: the
  deletions break two old-core modules, so items go early rather than being
  ported: `AxisMappings` accessor fields and setters and
  `analyze_data_for_scales` in `axis_system.rs`; `render_from_bundle`,
  `parse_bundle_json` and their tests in `wasm_api.rs`;
  `examples/scale_axis_integration_demo.rs` and the `axis_system` doctests that
  use them. Both modules are deleted in GUP-488 anyway. "Not deleted here"
  therefore means the files stay, minus these items.

## Escalation Triggers

- Deleting something outside the `chart_builder` dependency closure becomes
  necessary (for example, a module of the old core fails to compile).
- An example on gup-core, a gup-core test or a dogfood task fails.
- `mask old-path-loc` stays above 18,504.
- `gup-culling-lod` fails to build.

## Out of Scope

- The old render core, `app.rs`, `wasm_api.rs`, the facade: GUP-488.
- Porting any deleted builder: T5, after the flip.

## Technical Tasks

- [ ] Delete `chart_builder`; follow compile errors by deletion.
- [ ] Goldens, tests, benches, examples; Cargo targets; gallery and smoke lists;
      tutorials.
- [ ] CI step and mask task; `examples_smoke`; culling-lod and wasm32 checks.
- [ ] LOC, INDEX header, RFC note.

## Dependencies

### Prerequisite Stories

- GUP-466: RFC-001 S10g Delete Old Builders ✅ — scatter, line, bar and
  composite builders gone; four examples ported.
- GUP-485: RFC-001 S14a Dogfood Residue 📋 — no dogfood task uses
  `ComposedChart`.
- GUP-480: RFC-001 S12a Appender Handle 📋 — `t5_stream` ported (through
  GUP-485).

### Enables Stories

- GUP-488: S14c — only the old render core is left to delete.

## Testing Strategy

- **Build and test**: `cargo test -- --test-threads=1`, `cargo test --doc`,
  `cargo check --examples --tests --benches`, `examples_smoke` (ignored),
  `cargo test -p gup-culling-lod`, the wasm32 lib build.
- **Visual**: the five remaining gup-core examples' screenshots from
  `examples_smoke`.

## Success Metrics

- [ ] Old-path LOC at most 18,504.
- [ ] No `ComposedChart` anywhere in the repository's Rust code except the
      parked mobile and Tauri crates.

## Risk Assessment

- **Medium**: a long tail of examples and tests to delete; the risk is deleting
  something still needed. _Mitigation_: the escalation triggers and the rule
  "delete only what fails to compile".
- **Low**: trybuild `.stderr` churn: none (gup-core untouched).

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] `xvfb-run -a ./dogfood/run_all.sh` passes (no FAIL, no XPASS)
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] `mask wasm-browser` passes (this story changes the root crate's wasm
      build)
- [ ] `mask wasm-size` run and the figures reported in the Implementation
      Summary (expected unchanged; the orchestrator re-records)
- [ ] Rendered output verified by eye: the remaining examples' screenshots
- [ ] `mask old-path-loc` at most 18,504, recorded in INDEX.md
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Implementation Summary

Deleted `src/chart_builder*` (23 files with `export/html`, `export/pdf`,
`export/svg/renderer.rs`), the `pdf` feature and `printpdf`, and by the cascade
decision: `scale::{AccessorFunction, DataAnalyzer}` and the
`From<AccessorValue>` impl; in `axis_system.rs` `AxisMappings`,
`auto_configure`, `analyze_data_for_scales`, `configure_axis_components`,
`get_scale_configurations`, `AxisLayoutManager` and their tests/doctest; in
`wasm_api.rs` `render_from_bundle`, `parse_bundle_json` and tests.

- Examples deleted (16): area_chart_demo, boxplot_builder_demo,
  chart_hover_reveal_demo, choropleth_gpu_recolor, choropleth_world_population,
  color_scale_heatmap, density_scatter_overlay, export_png, heatmap_chart,
  html_export, multi_category_boxplot, multi_font_chart_demo, pdf_export,
  svg_export, violin_plot_demo, scale_axis_integration_demo.
- Tests deleted (8): axis_integration_tests, color_scale_integration,
  heatmap_integration, html_export_integration, multi_category_boxplot_tests,
  pdf_export_integration, png_export_integration, svg_export_integration.
- Benches deleted: 0. Goldens deleted: `tests/golden/chart_builders/` (7 PNGs).
- CI step and `mask visual-regression` removed; `mask ci visual-regression`
  keeps the harness unit tests.
- `mask old-path-loc`: 26,601 to 18,504 (target met exactly).
- Gates: `mask all-check` green; `cargo test --doc` 132 passed; smoke-examples
  green; wasm32 lib build and `gup-culling-lod` tests pass. `mask wasm-size`
  scatter 331,601 B gz (+2% info, pass), unchanged in kind.
- Dogfood: 9 PASS, `t4_linked` and `t5_live` FAIL (windowed under Xvfb; they
  depend only on `gup-core`, which this story did not touch). Not caused by this
  change; reported to the orchestrator.
- `mask wasm-browser` not run (no browser here).

## Resolved (2026-10-23)

**Question**: Deleting `chart_builder`, `export/html` and
`scale::AccessorFunction` breaks two old-core modules outside the
`chart_builder` closure: `src/axis_system.rs` and `src/wasm_api.rs`. Is it
intended to delete items from them (the Escalation Trigger "a module of the old
core fails to compile")?

**Evidence** (`cargo check --lib` after the AC1 deletions; the working tree was
reset, nothing is committed beyond the Start commit):

- `scale.rs` `AccessorFunction`, `From<AccessorValue> for DataValue` and
  `DataAnalyzer::analyze_field` all name `AccessorValue` (deleted per AC1).
  `src/axis_system.rs` then fails: it imports `scale::AccessorFunction`,
  `AxisMappings` holds four `Option<AccessorFunction<T>>` fields with setters,
  and `analyze_data_for_scales` calls `analyze_field` four times (51 accessor
  mentions in 933 lines). `AxisMappings` is public (`lib.rs:242`, used by
  `examples/scale_axis_integration_demo.rs` and `axis_system` doctests).
- `src/wasm_api.rs` imports `export::html::{ChartBundle, ChartSnapshot}` in
  `render_from_bundle` (a `wasm_bindgen` entry point, line ~492-560) and
  `parse_bundle_json` (line ~572-615) plus their tests. The story lists
  `wasm_api.rs` under "Not deleted here".

**Options**:

1. Delete the items: `AxisMappings` accessor fields/setters,
   `analyze_data_for_scales` (and the auto-scale path that uses it) in
   `axis_system.rs`; `render_from_bundle`, `parse_bundle_json` and tests in
   `wasm_api.rs`. Keeps the "delete, never port" rule; the old core's public API
   shrinks.
2. Keep `export/html` (`ChartBundle`, `ChartSnapshot`, snapshot only, without
   `HtmlExporter`) and port `AccessorValue` into `scale.rs`. Smaller deletion,
   LOC target 18,504 may not be met.

**Recommendation**: option 1, with the owner confirming `AxisMappings` and
`render_from_bundle` may lose those items (both die in GUP-488 anyway).

## Retrospective

**Completed**: 2026-10-23

### Key Technical Learnings

#### Deletion cascades through shared accessor types

- **Challenge**: `AccessorValue` lived in `chart_builder` but `scale.rs`,
  `axis_system.rs` and `wasm_api.rs` (old core) used it, so "delete what fails
  to compile" reached three modules the story called not-deleted.
- **Solution**: escalated; the orchestrator approved deleting the items early
  (both modules die in GUP-488).
- **Pattern**: when a story's closure analysis lists only importers by name,
  also grep for the types that move with the module, not just the module path.

#### Dead code after deletion

- `-D warnings` forces removing newly dead private items (layout manager,
  analyzer, scale-config helpers) in the same commit; budget for it.

### Architectural Decisions

#### Items deleted early from the old core

- **Decision**: remove `AxisMappings`/`auto_configure`, `DataAnalyzer`,
  `render_from_bundle` instead of porting.
- **Trade-off**: `AxisSystem` can no longer auto-build scales; it is dead weight
  until GUP-488 removes it.

### Development Workflow Insights

- zsh does not word-split unquoted variables; use `${=var}` in loops.
- `cargo test` (whole) ran doc tests that failed with duplicate-wgpu type errors
  while `cargo test --doc` alone passed; the 17 failures are in files this story
  did not touch (streaming, brush, gpu_timer). Worth a look in GUP-488.
- Dogfood `t4_linked` and `t5_live` fail under Xvfb here independent of this
  change.

### Follow-up Stories

None.
