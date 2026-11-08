---
implementer: sonnet
implementer-reason:
  two provided methods on ChartExt that mirror save_png over the existing
  SvgTarget; the shape is decided below
touches:
  [
    crates/gup-core/src/chart.rs,
    crates/gup-core/tests/svg.rs,
    docs/tutorials/01_getting_started.md,
    CHANGELOG.md,
  ]
---

# GUP-671: A Chart Saves Itself as SVG in One Call

## Story Overview

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: ✅
Complete 2026-11-08 **Created**: 2026-11-08

## Context

`chart.save_png(&cx, "chart.png", 640, 400)` is one call (`ChartExt` in
`crates/gup-core/src/chart.rs`). SVG takes three, with two crate-root imports
that are not in the prelude:

```rust
let resolved = chart.resolve(&cx, 640.0, 400.0)?;
let mut svg = SvgTarget::new();
svg.render(&resolved.scene)?;
svg.save("chart.svg")?;
```

The feature tour (GUP-669, chapter 1, "A chart in five lines") had to show this
and say "there is no `save_svg`". SVG is a headline output, and 0.1 is not yet
published, so the asymmetry is cheap to fix now.

## User Story

> "As a new user, I want `chart.save_svg(&cx, "chart.svg", 640, 400)` to work
> like `save_png`, so that both outputs are one line."

## Read First

- `crates/gup-core/src/chart.rs` (`ChartExt::render`, `save_png`).
- `crates/gup-core/src/svg.rs` (`SvgTarget::new`, `render`, `svg`, `save`) and
  `VectorTarget`.
- `crates/gup-core/tests/svg.rs`; the `svg` feature in
  `crates/gup-core/Cargo.toml` and the root `Cargo.toml`.

## Acceptance Criteria

- [x] Two provided methods next to `save_png`, both `#[cfg(feature = "svg")]`,
      with the signatures below, doc comments and a doctest for `save_svg`
      (writing to a temp dir).

```rust
fn to_svg(&mut self, cx: &Context, width: u32, height: u32) -> Result<String>;
fn save_svg(
    &mut self,
    cx: &Context,
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
) -> Result<()>;
```

- [x] `save_svg` creates missing parent directories exactly as `save_png` does
      (shared helper or the same code path; no second copy of the logic).
- [x] A test in `crates/gup-core/tests/svg.rs`: for a builder scatter, `to_svg`
      equals the three-call output byte for byte; `save_svg` writes the same
      string to a nested path that did not exist.
- [x] Tutorial 1's "save" section mentions `save_svg` beside `save_png` (one
      sentence and the call); its doctest or test still passes.
- [x] CHANGELOG `[0.1.0]` Added line.
- [x] `mask feature-matrix` passes (the methods vanish cleanly without `svg`).

## Decisions

- Methods on `ChartExt`, not free functions: they mirror `save_png`.
- `u32` width and height, as `save_png` takes; convert to `f32` for `resolve`
  inside.
- No HTML export in this story.

## Escalation Triggers

- `ChartExt` cannot carry a `#[cfg(feature = "svg")]` provided method without
  breaking an implementor or the feature matrix.
- The three-call output and `to_svg` differ for a reason that is not a bug.

## Out of Scope

- HTML export; a builder setter for a right-hand y axis; making `AxisFit` public
  (gaps the tour also found; separate stories if wanted).

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

`ChartExt::to_svg` and `save_svg` (feature `svg`) in
`crates/gup-core/src/chart.rs`. `save_svg` goes through `SvgTarget::save`, which
already creates parent directories (no second copy of that logic). Test
`chart_saves_itself_as_svg` in `tests/svg.rs`; tutorial 1 and CHANGELOG updated.

## Retrospective

**Completed**: 2026-11-08

- `SvgTarget::save` already made parent directories, so the AC was met by reuse.
- The two methods repeat three lines of resolve/render; a hidden helper on the
  public trait was rejected as extra API.
