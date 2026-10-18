---
implementer: sonnet
implementer-reason:
  deletion by a fixed rule (delete what no longer compiles, port nothing) with
  the CI and gallery edits and the LOC target fixed in Decisions
---

# GUP-486: RFC-001 Step S14b: Delete the Old Chart Builder Module

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-19

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
