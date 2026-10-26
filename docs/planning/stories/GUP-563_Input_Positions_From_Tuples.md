---
implementer: sonnet
implementer-reason:
  additive conversions and constructors with signatures and the doc rewrites
  fixed in Decisions
touches: [crates/gup-core/src/plot.rs]
---

# GUP-563: Input Positions From Tuples

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 🚧 In
Progress **Created**: 2026-10-26

## Context

Gup has two types named `Point`. The prelude's `Point` is the point scale
(`scale::Point`, the ordinal sibling of `Band`); `gup::geom::Point` is a
position in logical pixels, which `Input`, `Plot::zoom`, `Plot::invert` and
every `pick` take. A reader of tutorial 4 must write `use gup::geom::Point;`
beside `use gup::prelude::*;` and hope the two do not collide (GUP-532
retrospective). Five of the tutorial's blocks begin with that import
(`docs/tutorials/04_interactions.md` lines 59, 128, 202, 253, 287, 333), as do
tutorial 6 (line 248) and the `Chart` trait's doctest (`chart.rs` line 25).

Renaming either type breaks more than it fixes: `Band`/`Point` is the standard
pair of ordinal scales, and `geom::Point` appears in `Layout` and the
integrations. Instead this story lets user code pass a plain `(f32, f32)`
wherever a position goes in, so it never has to name `geom::Point`.

Before:

```rust
use gup::geom::Point;
use gup::prelude::*;
use gup::{Input, Layout, Response};

fn pixel(layout: &Layout, x: f64, y: f64) -> Point {
    // ...
    Point::new(along(&layout.x_ticks, x), along(&layout.y_ticks, y))
}
let response = chart.handle(Input::PointerMoved { at: kea });
```

After:

```rust
use gup::prelude::*;
use gup::{Input, Layout, Response};

fn pixel(layout: &Layout, x: f64, y: f64) -> (f32, f32) {
    // ...
    (along(&layout.x_ticks, x), along(&layout.y_ticks, y))
}
let response = chart.handle(Input::pointer_moved(kea));
```

## Read First

- `crates/gup-core/src/geom.rs` lines 1–21 (`Point`).
- `crates/gup-core/src/input.rs` lines 1–60 (`Input`, `Button`).
- `crates/gup-core/src/plot.rs`: `zoom` (line 872), `invert` (line 1067),
  `pick_row` (line 1698), `pick` (line 1736).
- The builders' `pick` methods: `builder/scatter.rs` line 343, `line.rs` 400,
  `bar.rs` 942, `histogram.rs` 432, `heatmap.rs` 254.
- `docs/tutorials/04_interactions.md` (the six blocks named above),
  `docs/tutorials/06_custom_marks.md` lines 240–380, `chart.rs` lines 15–50.

## User Story

> "As a developer driving a chart with input, I want to pass `(x, y)` pixel
> positions directly, so that I never import a second `Point` that clashes with
> the prelude's."

## Acceptance Criteria

### AC1: Conversions

- [ ] `impl From<(f32, f32)> for geom::Point` and
      `impl From<[f32; 2]> for geom::Point`, with a doctest:
      `let p: Point = (3.0, 4.0).into();`.

### AC2: Input constructors

- [ ] `Input::pointer_moved`, `pointer_down`, `pointer_up` and `wheel` exist
      (signatures in Decisions), each with a one-line doc, and a unit test in
      `input.rs` shows each equals the struct-literal variant.

### AC3: Methods that take a position accept tuples

- [ ] `Plot::zoom`, `Plot::invert`, `Plot::pick_row`, `Plot::pick`, and every
      builder's `pick` take `at: impl Into<Point>`. A compile-pass test
      (`tests/compile_pass/positions_from_tuples.rs`) calls each with a
      `(f32, f32)` and with a `geom::Point`, and uses only
      `gup_core::prelude::*` and `gup_core::Input`.
- [ ] `Chart::pick_row` and `Chart::handle` (trait methods) are unchanged, so
      `Box<dyn Chart>` still compiles (`tests/pick.rs` line 466 unchanged).

### AC4: Docs no longer import `geom::Point`

- [ ] `rg -n 'geom::Point' docs/tutorials README.md src/lib.rs crates/gup-core/src/chart.rs`
      prints nothing. Tutorial 4's blocks use tuples and the constructors as in
      the "After" example above; tutorial 6's block builds its positions as
      tuples; the `Chart` doctest uses `Input::wheel(at, 1.0)`.
- [ ] Tutorial 4's prose that introduces `pixel` says it returns "the pixel
      position as `(x, y)`".
- [ ] `cargo test --doc -p gup` and
      `scripts/with_cargo_env.sh cargo test -p gup-core --doc` pass, and
      `tests/tutorials.rs` goldens are unchanged.

## Decisions

- **Conversions** live in `geom.rs`. No `From<(f64, f64)>`: positions are `f32`
  everywhere in `Layout`, and two impls would make float literals ambiguous.
- **Constructors**, in `input.rs`:

  ```rust
  impl Input {
      pub fn pointer_moved(at: impl Into<Point>) -> Self;
      pub fn pointer_down(at: impl Into<Point>, button: Button) -> Self;
      pub fn pointer_up(at: impl Into<Point>, button: Button) -> Self;
      pub fn wheel(at: impl Into<Point>, lines: f32) -> Self;
  }
  ```

  `PointerLeft` needs none. The variants stay public and constructible, so hosts
  (`web.rs`, `gup-egui`, `gup-bevy`) are untouched.

- **Inherent methods** switch to `at: impl Into<Point>` (source-compatible for
  callers passing a `Point`); trait methods do not, because `Chart` is used as a
  trait object.
- **No rename** of either `Point` (see Context), and `geom::Point` is not added
  to the prelude under another name.

## Escalation Triggers

- Changing an inherent method to `impl Into<Point>` breaks type inference at an
  existing call site (for example a call passing `Default::default()`).
- A trybuild `.stderr` snapshot changes.

## Out of Scope

- Adding `Input`, `Button` or `Response` to the prelude (a separate decision).
- The integrations' event conversion code.

## Technical Tasks

- [ ] `From` impls and doctest in `geom.rs`.
- [ ] `Input` constructors and unit test.
- [ ] `impl Into<Point>` on `Plot` and builder methods; compile-pass test.
- [ ] Rewrite tutorial 4, tutorial 6 and the `Chart` doctest blocks.

## Dependencies

### Prerequisite Stories

- GUP-532: Tutorials Interaction and Streaming ✅ — tutorial 4 as doctests.

### Enables Stories

- GUP-536: Tutorial Tour of Chart Types — its blocks can use tuples.

## Testing Strategy

- **Unit tests**: constructors equal the literals.
- **Integration tests**: the compile-pass file; the tutorials' doctests and
  goldens.
- **Visual validation**: none needed (no rendering change; tutorial goldens
  unchanged).

## Success Metrics

- [ ] Zero `geom::Point` imports in user-facing docs.
- [ ] No breaking change for existing callers.

## Risk Assessment

- **Low**: inference regressions at call sites. _Mitigation_: the escalation
  trigger; `cargo check --examples` and the dogfood crate cover external call
  sites.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`, plus
      `scripts/with_cargo_env.sh cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`, and the dogfood crate
      builds (`cargo check --manifest-path dogfood/Cargo.toml`)
- [ ] Rendered output verified by eye: not needed, no output change
- [ ] `mask wasm-browser`: the scatter page drives `Input`; run it
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
