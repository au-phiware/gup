---
implementer: sonnet
implementer-reason:
  the cause is found (the size harness calls SvgTarget); the fix is a harness
  feature, a metric line and removing cfg attributes, all named in Decisions
touches:
  [crates/gup-core/src/svg.rs, crates/gup-core/wasm-size/, scripts/wasm_size.sh]
---

# GUP-557: Keep SvgTarget Text Out of the WASM Build

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 🚧 In
Progress Planned **Created**: 2026-11-01 (made decision-complete 2026-11-02)

## Context

GUP-580 added fallback fonts to `SvgTarget`. `mask wasm-size` then measured
+1,329 B gz on the reference scatter, because `SvgTarget::write` is linked into
the WASM build. GUP-580 put the fallback-only code behind the `svg` feature,
which the harness builds without
(`default-features = false, features = ["web"]`). That brought the cost to +406
B. The price is a behaviour split: without `svg`, the document names the primary
font family only, does not embed fallbacks, and anchors fallback text with the
primary font's origins (GUP-580 retrospective, "Guides-only builds name one
family").

The cause is now known. Nothing in `gup-core` drags `SvgTarget` into a WASM
binary. The size harness calls it on purpose:
`crates/gup-core/wasm-size/scatter/src/lib.rs`, in `render_scatter` (about line
125), does `let mut svg = SvgTarget::new(); svg.render(&scene.guides())`. Its
comment says the budget "measures the SVG guides path as recorded". A browser
app that never exports SVG does not link `SvgTarget::write`; the linker drops
it. So `wasm.scatter.gz_bytes` measures a browser chart _plus_ an SVG export,
and the `svg`-feature gating on fallbacks only trims that optional export.

So the fix is in the measurement. `wasm.scatter` measures a browser chart. The
SVG export gets its own number, as `wasm.per_row_type` does for one more row
type. With that, the fallback code can be ungated, and SVG output is the same
with or without the `svg` feature, apart from data marks.

## Read First

- `crates/gup-core/wasm-size/scatter/src/lib.rs` lines 1–30 and 118–132 (the SVG
  call), and its `Cargo.toml` `[features]` (`extra-row-type`).
- `scripts/wasm_size.sh` lines 55–90: `measure`, the three builds and the
  `metric` lines.
- `crates/gup-core/src/svg.rs` lines 195–262 (`used_fallbacks`, the `fallbacks`
  binding in `write`) and 460–500 (`chain`, `fallback_families`). Lines 794–810
  hold the `a_repeated_fallback_name_gets_its_chain_index` test.
- `crates/gup-core/PERF_BUDGETS.md` lines 80–86 (the `wasm.*` rows) and the
  GUP-450 note on `wasm.per_row_type` (the pattern for a difference metric).
- GUP-580's "Resolved (2026-11-01)" section and retrospective.

## User Story

> "As a web developer, I want the WASM size budget to measure what a browser
> chart costs, and SVG export to behave the same in every build, so that the
> number is honest and the output is consistent."

## Acceptance Criteria

### AC1: Parent measurement

- [ ] Before any change, run `mask wasm-size` on the parent commit. Record
      `wasm.scatter.gz_bytes` and `wasm.per_row_type.gz_bytes`.

### AC2: SVG export behind a harness feature

- [ ] The scatter harness gains feature `svg-export`, off by default. The
      `SvgTarget` call in `render_scatter` (and its `use` of `SvgTarget` and
      `VectorTarget`, and `web_log(svg.svg().len())`) compiles only with it. The
      comment above it says why: the default build measures a browser chart;
      `svg-export` measures the SVG guides export on top.
- [ ] `scripts/wasm_size.sh` builds the scatter three times, in this order:
      `--features extra-row-type`, then `--features svg-export`, then the
      default. The default build comes last so that `wasm_browser.sh` loads it.
      It prints
      `metric wasm.svg_export.gz_bytes <svg-export gz − default     gz>` with
      the others. The script's header comment describes the third build in one
      sentence.
- [ ] `mask wasm-size` after the change: `wasm.scatter.gz_bytes` is lower than
      AC1's by at least 1,000 B. Report the parent value, the new value and
      `wasm.svg_export.gz_bytes`. `perf_budget.sh` reports the scatter row as
      "pass (faster than recorded: re-record?)" or "pass". The worker does not
      edit `PERF_BUDGETS.md`; the orchestrator re-records the scatter and adds a
      `wasm.svg_export.gz_bytes` row with check `info`.
- [ ] `mask wasm-browser` passes. The page never checked the SVG length; the
      browser loads the default build.

### AC3: Fallbacks ungated

- [ ] In `svg.rs`, `used_fallbacks`, `chain` and `fallback_families` each have
      one definition with no `cfg`, and `write`'s `fallbacks` binding is
      `&scene.fallback_fonts` unconditionally. The
      `#[cfg(not(feature = "svg"))]` stand-ins and their doc comments
      ("Guides-only builds … name the primary family only") are deleted, and so
      is the `#[cfg(feature = "svg")]` on
      `a_repeated_fallback_name_gets_its_chain_index`.
- [ ] Gating that is about data marks (`MarkBatch`, `LayerVector`,
      `write_layer`, the data-layer error in `render`) is unchanged.
      `git diff crates/gup-core/src/svg.rs` touches only fallback code.
- [ ] `cargo clippy -p gup-core --lib --no-default-features -- -D warnings`
      passes (it is in `mask all-check`).
      `cargo test -p gup-core --no-default-features --features window --lib svg::`
      passes: `svg.rs`'s unit tests run without `svg`. If the lib's tests do not
      compile without `svg` because of code outside `svg.rs`, record the error
      in the Implementation Summary and skip this command; do not fix other
      modules here.
- [ ] The default scatter is unchanged by AC3. Run `mask wasm-size` again after
      AC3. `wasm.scatter.gz_bytes` equals AC2's value exactly, since the default
      build no longer links SVG code. `wasm.svg_export.gz_bytes` grows by about
      GUP-580's 923 B (1,329 − 406), and that is accepted.

## Decisions

- **Move the SVG export out of the measured scatter; do not gate more code.**
  The +1,329 B was never paid by browser apps that do not export SVG; only the
  harness paid it. Gating more of `svg.rs` behind `svg` would split behaviour
  further for no user benefit.
- **`wasm.svg_export.gz_bytes` is `info`, with no cap.** It is a difference of
  two builds, like `wasm.per_row_type`. The orchestrator decides whether to give
  it a cap later, in `PERF_BUDGETS.md`.
- **The 400 KB `wasm.over_wgpu` cap is unchanged.** It is now measured without
  SVG export, so its headroom grows by the export's cost. That is stated in the
  orchestrator's re-record note (AC2), not hidden.
- **Ungate fallbacks everywhere.** A guides-only build (no `svg`) then names and
  embeds fallback families exactly as an `svg` build does. That removes
  GUP-580's trade-off.
- **No API change.** `SvgTarget`, `SvgOptions`, `VectorTarget` and the `svg`
  feature keep their meaning.

## Escalation Triggers

- After AC2, `wasm.scatter.gz_bytes` drops by less than 1,000 B. That means
  something other than the harness call keeps SVG code linked. Run
  `mask wasm-attrib` and report the functions from `svg.rs` it lists, with their
  sizes. Do not gate them.
- AC3 changes the default scatter's size at all.
- A test asserts the guides-only single-family behaviour. List it; it is
  expected to be updated, but report it first.

## Out of Scope

- Re-recording `PERF_BUDGETS.md` and adding the new row: the orchestrator.
- SVG title and description: GUP-593, which edits `write` after this story.
- Any change to what the `svg` feature gates for data marks.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30
      minutes.
- [ ] Parent measurement (AC1).
- [ ] Harness feature, `wasm_size.sh`, measurement, browser check (AC2). Commit.
- [ ] Ungate the fallback code; clippy and unit tests without `svg`; measure
      again (AC3). Commit.

## Dependencies

### Prerequisite Stories

- GUP-580: Fallback fonts in gup-core and SVG ✅ — the gated code.

### Enables Stories

- GUP-593: SVG title and canvas `aria-label` — edits the same `write` code and
  the harness afterwards.

## Testing Strategy

- **Size**: `mask wasm-size` on the parent, after AC2 and after AC3.
- **Unit tests**: `svg.rs`'s tests with and without the `svg` feature.
- **Integration**: `cargo test -p gup-core -- --test-threads=1` (the SVG and
  fallback tests); `mask wasm-browser`.

## Success Metrics

- [ ] `wasm.scatter.gz_bytes` measures a browser chart without SVG export, and
      the export's cost is its own number.
- [ ] SVG output names and embeds fallbacks in every build.

## Risk Assessment

- **Low**: removing the SVG call from the default harness hides a future SVG
  size regression. _Mitigation_: `wasm.svg_export.gz_bytes` is printed on every
  `mask wasm-size` and recorded as an `info` row.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not applicable; SVG goldens and parity
      checks pass unchanged
- [ ] `mask wasm-browser`: required (the harness changes)
- [ ] WASM size: `mask wasm-size` parent, after AC2 and after AC3
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
