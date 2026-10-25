---
implementer: sonnet
implementer-reason:
  docs rewrite of two tutorials against existing, doctested APIs (wgsl_function,
  derive(Mark)); conventions fixed by GUP-530
touches: [src/lib.rs]
---

# GUP-531: Tested Tutorials, Part 2: Shader Functions and Custom Marks

## Story Overview

**Initiative**: Strategic Review 2026-11 (N1 Docs and Tutorials) **Status**: ✅
Complete (2026-10-25) **Created**: 2026-10-24

## Context

Tutorials 3 (custom shader functions) and 6 (custom marks) describe deleted
APIs: `ComposableShaderFunction`, the `wgsl_function!` macro, a manual `Mark`
impl with vertex types and `MarkRegistry`. Today a user writes WGSL in their own
crate with `#[wgsl_function]` on an `impl ShaderFn` block (checked when their
crate compiles) and a mark with `#[derive(Mark)]` plus an inline or file WGSL
module (`crates/gup-core/tests/derive_mark.rs` is a mark defined from outside
the crate). These are Gup's distinguishing features, so their tutorials matter
most to the users who would choose Gup over a CPU plotting crate.

The strategic review also asks for "a `#[derive(Mark)]` guide" (the old
`CUSTOM_MARK_GUIDE.md` was deleted by GUP-499). This story makes tutorial 6 that
walkthrough and puts the attribute reference on the `Mark` trait's rustdoc,
where `cargo doc` users find it.

## Read First

- GUP-530's Decisions (block conventions, goldens in `tests/tutorials.rs`).
- `crates/gup-core/src/lib.rs` lines 190–330 (`wgsl_function`'s docs, the
  earthquake-energy example and "What you write").
- `crates/gup-core/src/encoding.rs`: the `ShaderFn` trait docs, `CpuMirror`,
  `EncodeFn::then` (search `fn then`).
- `crates/gup-core/tests/wgsl_function.rs` lines 1–80.
- `crates/gup-core/tests/derive_mark.rs` (whole file) and
  `crates/gup-core/src/channel.rs` lines 270–360 (`Antialias`, `Geometry`,
  `HitShape`, `Mark`).
- `crates/gup-core-macros/src/mark.rs` lines 18–30 (`USAGE`) and 140–280 (the
  attribute checks).
- `crates/gup-wgsl/library/circle.wgsl` (the mark contract by example) and
  `coverage.wgsl` (helpers a mark imports).
- `docs/tutorials/03_custom_shader_functions.md`, `06_custom_marks.md` as they
  are (learning path only).

## User Story

> "As a developer with a domain-specific encoding (a seismic energy scale, a
> custom glyph), I want tutorials that walk me through writing my own WGSL
> shader function and mark, so that I can extend Gup without forking it."

## Acceptance Criteria

### AC1: Tutorial 3, Shader Functions

- [x] Covers, in this order: what a shader function is (a WGSL function with an
      f64 `CpuMirror`, so axes, legends, picking and SVG agree with the GPU);
      composing built-ins with `.then(..)` (e.g. `Log` then `Sequential`);
      writing one with `#[wgsl_function]` (inline `wgsl = r#"…"#`, `Params`,
      `params()`, a `CpuMirror`, `monotonic`); using it on a channel with
      `.attr(Circle::FILL, MyFn.encode(..))`; a `rust,compile_fail` block
      showing a WGSL error caught at compile time; and `file = "…"` for WGSL in
      a file (shown in prose, since a doctest has no crate files).
- [x] It embeds one golden image: a scatter coloured through the user function.
- [x] Every `#[wgsl_function]` block sets `path = "…"` (a doctest's crate is
      named after `gup`, which owns the `gup::` namespace), as `src/lib.rs`'s
      doctest does.

### AC2: Tutorial 6, Custom Marks

- [x] Covers, in this order: the mark contract (`<Name>In`, `Varyings` with
      `clip`, `vertex`, `shade` returning premultiplied colour); a `Square` mark
      with `#[derive(Mark)]` and inline WGSL using `gup::coverage`; channel
      roles (`X`, `Y`, `Size`, `Width`, `Height`, `Color`) and defaults, and
      what each role does (axis closures, domain fits, overhang, legends); the
      generated setters (`SquareAttrs`); drawing it in a `Plot`; making it
      pickable (`hit = AlignedBox`, then `gpu_pick` with
      `hit_distance`/`hit_row`); and `geometry = Polyline` and
      `antialias = Msaa` in one paragraph each.
- [x] One `rust,compile_fail` block shows the derive rejecting a mismatched
      `SquareIn` struct.
- [x] It embeds one golden image of the squares.

### AC3: The derive reference

- [x] The `Mark` trait's rustdoc (`crates/gup-core/src/channel.rs`) gains a
      "Deriving" section: a table of every `#[mark(..)]` key (`module`, `wgsl`,
      `file`, `path`, `geometry`, `antialias`, `hit`, `gpu_pick`) and every
      `#[channel(..)]` key (`role`, `default`), each with its allowed values and
      one line of meaning, matching `USAGE` and the macro's checks, and a link
      to tutorial 6.
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc -p gup-core --no-deps` passes.

### AC4: Wiring

- [x] `src/lib.rs`'s `mod tutorials` gains `ShaderFunctions` and `CustomMarks`;
      `cargo test --doc -p gup -- tutorials` passes; banners removed; the
      GUP-530 `awk` and `grep` checks print nothing for `03_*.md` and `06_*.md`.
- [x] Goldens `tutorials/03_custom_ramp` and `tutorials/06_squares` exist in
      `tests/tutorials.rs`, read by eye and described.
- [x] `docs/tutorials/assets/tutorial06_custom_mark.png` is deleted;
      `docs/tutorials/README.md`'s rows for tutorials 3 and 6 are updated.

## Decisions

- **Follow GUP-530's conventions** (complete programs, no hidden lines, no
  `ignore`, images are goldens).
- **Tutorial 6 replaces the custom-mark guide.** There is no separate
  `docs/CUSTOM_MARK_GUIDE.md`; reference material goes on the `Mark` trait's
  rustdoc (AC3), so it is versioned with the macro and checked by `cargo doc`.
- **The `Square` mark** is the one in `tests/derive_mark.rs` (same WGSL), so the
  tutorial's mark is already proven to work from outside the crate.
- **Compile-fail blocks** are `rust,compile_fail` without an error code (the
  derive's and naga_oil's messages are not stable error codes).

## Escalation Triggers

- A `#[wgsl_function]` or `#[derive(Mark)]` doctest in the `gup` crate fails to
  resolve `::gup_core` or the `gup::` import namespace (GUP-487 made the macros
  resolve the crate path): report the error; do not add a `gup-core` dependency
  to the root crate.
- The `compile_fail` block passes to compile for an unrelated reason (for
  example a missing import): it must fail on the WGSL or the struct mismatch;
  check by temporarily removing the deliberate error.

## Out of Scope

- New mark features (an `Area` mark is GUP-545).
- Tutorials 1, 2, 4, 5 (GUP-530, GUP-532).

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] Rewrite tutorial 3; wire it; add its golden; commit.
- [x] Rewrite tutorial 6; wire it; add its golden; commit.
- [x] Write the `Mark` "Deriving" section; run `cargo doc` with `-D warnings`.
- [x] Update the tutorials README; run the DoD commands.

## Dependencies

### Prerequisite Stories

- GUP-530: Tested tutorials part 1 📋 — `mod tutorials`, `tests/tutorials.rs`
  and the conventions.

### Enables Stories

- None directly.

## Testing Strategy

- **Doctests**: `cargo test --doc -p gup -- tutorials`, including the two
  `compile_fail` blocks.
- **Integration**: `cargo test -p gup --test tutorials -- --test-threads=1`.
- **Visual validation**: the two new goldens, read by eye.

## Success Metrics

- [x] Both tutorials' code is compiled by CI; the derive's attributes are
      documented in one place that `cargo doc` checks.

## Risk Assessment

- **Low**: user WGSL in doctests compiles slowly (naga*oil at macro expansion).
  \_Mitigation*: two or three such blocks only; record the doctest time.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (the two tutorial goldens)
- [x] `mask wasm-browser`: not needed, no layout, input or wasm change
- [x] WASM size: unchanged, no library code change (rustdoc only)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `docs/tutorials/03_custom_shader_functions.md` rewritten: what a shader
  function is, `Log.then(Sequential)`, a user colour ramp `Ember`
  (`#[gup::wgsl_function]`, `Params`, `params()`, `CpuMirror`) on
  `Circle::FILL`, an `Energy` function that starts a chain (`monotonic`), a
  `compile_fail` WGSL syntax error, and `file = "…"` in prose.
- `docs/tutorials/06_custom_marks.md` rewritten: the mark contract, a `Square`
  (the WGSL of `tests/derive_mark.rs`), channel roles, generated setters, a
  pickable `Tile` (`hit = AlignedBox`, `gpu_pick`, CPU and GPU picks agreeing),
  `Polyline` and `Msaa` in a paragraph each, and a `compile_fail` struct/WGSL
  mismatch.
- `Mark`'s rustdoc (`crates/gup-core/src/channel.rs`) gained "Deriving": a table
  of every `#[mark(..)]` and `#[channel(..)]` key.
- `src/lib.rs` includes both tutorials; `tests/tutorials.rs` has
  `tutorial_3_custom_ramp` and `tutorial_6_squares`; goldens
  `tests/golden/tutorials/03_custom_ramp.png` and `06_squares.png`;
  `tutorial06_custom_mark.png` deleted; README rows updated;
  `scripts/test_pre_commit.sh`'s "docs-only" case moved to tutorial 04.
- Tests: 7 new doctests (ShaderFunctions 3 run + 1 compile_fail, CustomMarks 2
  run + 1 compile_fail), 2 golden tests. `cargo test --doc -p gup -- tutorials`:
  20 passed + 3 compile_fail; doctest compile of the merged bundle about 12 s.
- `RUSTDOCFLAGS="-D warnings" cargo doc -p gup-core --no-deps` passes.

## Retrospective

**Completed**: 2026-10-25

### Key Technical Learnings

#### Doctest names and the `gup::` namespace

- **Challenge**: a doctest crate is named after `gup`, and the macros refuse a
  default module path in `gup::`.
- **Solution**: every `#[wgsl_function]` and `#[derive(Mark)]` block sets
  `path = "tutorialN_…"`; the prose explains why and says to omit it in a real
  project. `::gup_core` resolved without a dependency, as GUP-487 intended.

#### `hit = AlignedBox` needs six channels

- **Challenge**: the story's `Square` has `size`, but `AlignedBox` reads
  `x, y, width, height, x_end, y_end` by name.
- **Solution**: tutorial 6 keeps `Square` for drawing and introduces `Tile` (a
  `Rect` with only a fill) for picking, and says why. This also shows the
  `Width`/`Height` roles in a real block.

#### A `compile_fail` block can fail for the wrong reason

- **Challenge**: my first mismatch block also had `default = Color::BLACK` on a
  `Px` field, a second, unrelated E0308.
- **Solution**: removed the default, compiled the block as an example to read
  the one remaining error, and compiled the corrected block as `rust` to prove
  it passes. Same check for the WGSL block in tutorial 3.

### Architectural Decisions

#### Tile alongside Square

- **Decision**: two marks in tutorial 6 instead of bending `Square`.
- **Reasoning**: AC2 wants `Square` identical to `derive_mark.rs`, and picking
  requires the Rect channel names.
- **Trade-off**: about 80 more lines of WGSL on the page.

### Development Workflow Insights

- A fresh worktree was behind `main` (no `tests/tutorials.rs`); I merged `main`
  before starting.
- A chain's automatic domain is fitted at resolve, so `chain.eval(..)` before
  rendering uses an unfitted domain; the first draft asserted it and failed.
- The worktree guard refused long heredocs piped into `>>`; writing the snippet
  with the file tool and `cat`-ing it worked.

### Follow-up Stories

None written. Small observations: the derive's `role` error message says "one of
`X`, `Y`, `Size` or `Color`" but `Width` and `Height` are accepted
(`crates/gup-core-macros/src/mark.rs`); `docs/gallery/index.html` still lists
`tutorial06_custom_marks` as an example that does not exist.
