---
implementer: sonnet
implementer-reason:
  the rename and the new home are decided below; the work is a sweep whose
  compile errors name every site, with a few strings that tests pin
touches:
  [
    crates/gup-core/src/lib.rs,
    crates/gup-core/src/scale/band.rs,
    crates/gup-core/src/scale/mod.rs,
    crates/gup-core/tests/compile_fail,
    docs/tour,
    docs/tutorials,
    docs/TECHNICAL_APPROACH.md,
    gup-egui/examples/tour.rs,
    tests/tour.rs,
    tests/tutorials.rs,
    CHANGELOG.md,
  ]
---

# GUP-700: One `Point`

## Story Overview

**Initiative**: Strategic Review 2026-11 (first outside user, before 0.1)
**Status**: 📋 Ready **Created**: 2026-11-09

## Context

The first outside user wrote `use gup::Point;` and then
`Input::pointer_moved(Point::new(x, y))`, which fails:
"`geom::Point: From<gup::Point>` is not satisfied". `gup::Point` is the **point
scale** (keys to evenly spaced points, `scale/band.rs` line 307, re-exported at
the root in `lib.rs` line 219 and in the prelude at line 413). The type every
input and pick method takes is the **geometry** `gup::geom::Point` (`geom.rs`
line 8: an `f32` pair in logical pixels):
`Input::pointer_moved(impl Into<Point>)`, `pointer_down`, `pointer_up`, `wheel`,
`Chart::pick_row`, and `Chart::pixel` returns one.

Checked against `main` (2026-11-09):

- The prelude holds the geometry `Rect` (`lib.rs` line 413) but not the geometry
  `Point`, and holds the scale `Point`. So the prelude, which hosts glob-import,
  makes the wrong guess available for the one name every input call needs.
- GUP-685's story says `Point` "is `gup_core::geom::Point` ..., already in the
  prelude". That was wrong: the prelude's `Point` has always been the scale. The
  code in that story never names `Point` through the prelude, so nothing broke.
- Today the tour, tutorial 4 and `gup-egui` work around it with an explicit
  `use gup::geom::Point;` (`docs/tour/snippets/08_interaction.rs` line 7,
  `tests/tutorials.rs` line 308, `gup-egui/src/input.rs`). An explicit import
  shadows a glob, so those compile, and they would keep compiling after this
  story.
- The scale is rare: its uses are `scale/band.rs`, its conformance tests, one
  doctest in `encoding.rs` (line 351), prose in `lib.rs`, `scale/mod.rs`,
  `text_layer.rs`, `docs/TECHNICAL_APPROACH.md` (lines 106 and 189) and the
  tour's chapter 3 table (`docs/tour/03_encodings.md` line 38). The geometry
  type appears in 57 files.
- 0.1 is unpublished, so a rename needs no shim and no deprecation.

## Read First

- `crates/gup-core/src/scale/band.rs`: lines 295 to 330 (`Point`, its doctest
  and `padding`), line 451 (`ordinal_scale!(Point)`), the tests at 495 to 510.
- `crates/gup-core/src/lib.rs`: lines 205 to 225 (root re-exports), 405 to 420
  (the prelude), line 23 (crate docs), `pub mod geom` at line 152.
- `crates/gup-core/src/geom.rs` lines 1 to 40.
- `crates/gup-core/src/scale/mod.rs`: line 33 (`pub use band::{..}`) and the doc
  lines 9, 116, 127, 147, 222, 429, 736.
- `crates/gup-core/tests/compile_fail/borrowing_accessor.stderr` (mentions the
  scale in a note; `encoding.rs` line 693 is where it comes from).
- `docs/tour/README.md` and `docs/tour/region.rs` (how snippets and pages are
  kept in sync: `tests/tour.rs` `pages_match_their_snippets`).

## User Story

> "As a developer driving a chart from my own input loop, I want `Point` in the
> prelude to be the point I pass to `Input::pointer_moved`, so that the first
> thing I try compiles."

## Acceptance Criteria

### AC1: The scale is `PointScale`; `Point` is the geometry

- [ ] `gup_core::scale::Point` is renamed `PointScale` (type, its `impl`s,
      `ordinal_scale!` invocation, error strings, docs). `Band` keeps its name.
      Re-exported at the root and in the prelude as `PointScale`.
- [ ] The root and the prelude export `Point` as `geom::Point` (the geometry),
      and nothing at the root or in the prelude is named `Point` otherwise.
      `gup::geom::Point` still resolves (a test uses both paths and asserts they
      are the same type).
- [ ] An external-crate doctest (on the prelude's doc block in `lib.rs`, using
      `gup`'s path) compiles
      `chart.handle(Input::pointer_moved(Point::new(160.0, 100.0)))` with only
      `use gup::prelude::*;`, and `PointScale::new().padding(0.5)`.
- [ ] No pick, input or position method changes signature.

### AC2: Every site is swept

- [ ] `cargo check --workspace --all-targets` and `cargo test --doc` compile
      with no use of the old scale name. Verify no stale name remains:

  ```bash
  git grep -nE "\bPoint::(new|padding)\(\)|Band or Point|\`Point\`|\[\`Point\`\]" \
    -- '*.rs' '*.md' ':!docs/planning' ':!docs/archive'
  ```

  Each hit is either the geometry type or is renamed. The messages that say
  "Band or Point" (`selection.rs` lines 1607 to 1632) become "Band or
  PointScale", and `crates/gup-core/tests/columns.rs` line 590 and any other
  test that pins that text follows.

- [ ] The tour: chapter 3's table row (`docs/tour/03_encodings.md` line 38)
      reads `PointScale` and `gup_core::PointScale`; any snippet or page that
      names the scale follows, and `pages_match_their_snippets` passes.
- [ ] `use gup::geom::Point;` (or `gup_core::geom::Point`) imports that exist
      only because the prelude lacked it are removed from the tour snippets
      (`docs/tour/snippets/08_interaction.rs`), `tests/tutorials.rs`, tutorial
      4's code, `docs/TECHNICAL_APPROACH.md` line 333 and
      `gup-egui/examples/tour.rs`; imports in files that do not glob the prelude
      stay. Tour pages that quote a snippet are updated by the same edit (the
      snippet is the source; the test checks the page matches).
- [ ] `compile_fail` snapshots: re-bless with the project's task if a `.stderr`
      changes (`mask` has the task; find it with `mask --help`), and read the
      diff: only the name `Point` may change in a note.
- [ ] CHANGELOG `[0.1.0]` Changed: the scale `Point` is now `PointScale`, and
      the prelude's and root's `Point` is the geometry type; glob importers that
      used the scale must rename it.
- [ ] No golden `.png` or `.svg` changes.

## Decisions

- **Rename the scale, not the geometry.** The geometry `Point` is on the
  signature of every input and pick method, of `Chart::pixel`'s return and of
  `Rect`'s neighbour in the prelude; 57 files use it and hosts type it by hand.
  The scale has nine uses. Renaming the rarer name touches the fewer places and
  frees the common one.
- **`PointScale`, not `Dots`, `Ordinal` or `PointAxis`.** It is the name of the
  d3 scale this one mirrors, it says what it is, and `Ordinal` already names the
  concept both `Band` and this scale belong to (`ordinal_png.rs`). The suffix
  makes it the odd one out beside `Band`, `Linear` and `Log`; that is the price
  of not renaming the geometry, and the doc comment says so once.
- **The geometry `Point` goes in the prelude and at the root**, beside `Rect`.
  `gup::geom::Point` stays valid. `geom::Rect` stays where it is.
- **No alias, no deprecation shim**: 0.1 is unpublished.
- **`BandParams`, the WGSL module names and the `Slots` type are not renamed**
  unless a compile error forces it; the story is about the public name.
- **Files outside the repository's compile set** (`docs/planning/`,
  `docs/archive/`, strategic review pages, story files) are history and are not
  edited.

## Escalation Triggers

- A glob-importing file in the repository defines or imports its own `Point` in
  a way that now conflicts (an explicit import wins over a glob, so this should
  not happen; a local `struct Point` also shadows, so
  `examples/basic/01_hello_chart.rs` should compile unchanged).
- A tour page and its snippet differ after the edit and the page test cannot be
  satisfied without changing the golden image.
- `Rect`'s or `Point`'s `Hash`, `Eq` or conversion impls would need to change.
- A test pins the string `Point` in a message in a way the sweep above did not
  find.

## Out of Scope

- Renaming `Window` (GUP-701).
- Changing `geom::Point`'s fields or adding conversions.
- A `Size` type, or moving `geom` to the root.

## Technical Tasks

- [ ] Rename the scale; fix `scale/mod.rs`, `lib.rs` (both lists and the crate
      docs), `encoding.rs` doctest and note, `selection.rs` messages,
      `text_layer.rs`.
- [ ] Add the geometry `Point` to the root re-exports and the prelude.
- [ ] Sweep tour, tutorials, `TECHNICAL_APPROACH.md`, `gup-egui`, tests.
- [ ] Doctest, equality test, CHANGELOG.

## Dependencies

### Prerequisite Stories

- GUP-686: The Prelude Holds What Users Reach For ✅ — added `Input` and friends
  to the prelude; this finishes that list with the type they take.

### Enables Stories

- GUP-702: the embedding guide's input code names only the prelude.

## Testing Strategy

- **Unit tests**: `Point` through the root, the prelude and `geom` is one type;
  the `PointScale` unit tests in `band.rs` pass renamed.
- **Integration tests**: the tour and tutorial tests; `cargo check --examples`.
- **Visual validation**: none; no golden changes.
- **Performance**: not applicable.

## Success Metrics

- [ ] `use gup::prelude::*;` is enough to call
      `Input::pointer_moved(Point::new(x, y))`.
- [ ] No file in the repository imports the geometry `Point` by path only to
      work around the prelude.

## Risk Assessment

- **Low**: a rename inside an unpublished crate. _Mitigation_: the compiler
  finds every use; the grep finds the prose and the messages.
- **Medium**: a snapshot or message test hides a stale name. _Mitigation_: the
  grep in AC2.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not needed, no golden changes
- [ ] `mask wasm-browser`: not needed, a rename with no behaviour change
- [ ] WASM size: unchanged: no library code change beyond names
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
