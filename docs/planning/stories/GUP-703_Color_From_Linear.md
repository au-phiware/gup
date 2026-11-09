---
implementer: haiku
implementer-reason:
  two small pure functions and a move of two existing ones; signatures,
  clamping, tolerances and the test values are all given below
touches:
  [
    crates/gup-core/src/channel.rs,
    crates/gup-core/src/scale/sequential.rs,
    CHANGELOG.md,
  ]
---

# GUP-703: `Color::from_linear`

## Story Overview

**Initiative**: Strategic Review 2026-11 (first outside user, before 0.1)
**Status**: 📋 Ready **Created**: 2026-11-09

## Context

gup's `Color` is sRGB-encoded with straight alpha, "each component in
`0.0..=1.0`", and gup blends in that space (`channel.rs` lines 141 to 160). The
first outside user's renderer keeps its colours in linear light. To make a
chart's background match the rest of their scene they wrote the linear to sRGB
transfer function by hand, because `Color` has only `rgb` (sRGB components),
`hex`, `to_array` and `to_rgba8`.

Checked against `main` (2026-11-09): the two transfer functions already exist,
private to the colour-scale code: `srgb_to_linear` (`pub(crate)`) and
`linear_to_srgb` (private) at `scale/sequential.rs` lines 181 to 195, used by
the OKLab interpolation. They use the standard sRGB piecewise curve (threshold
0.04045 and 0.0031308). Nothing public exposes them, and `Theme::background` is
a sRGB `Color`, so a host with a linear clear colour needs the forward
conversion too.

## Read First

- `crates/gup-core/src/channel.rs` lines 141 to 205 (`Color`, `rgb`, `hex`,
  `to_array`, `to_rgba8`).
- `crates/gup-core/src/scale/sequential.rs` lines 181 to 220 (the two functions
  and their callers `srgb_to_oklab`, `oklab_to_srgb`).

## User Story

> "As a developer whose renderer works in linear light, I want to make a gup
> `Color` from linear components and read one back as linear, so that a chart's
> background and marks match my scene without my own transfer function."

## Acceptance Criteria

### AC1: The conversions

- [ ] `Color::from_linear`, `Color::from_linear_rgba` and `Color::to_linear`
      exist with the signatures under Decisions, in `channel.rs`.
- [ ] Known values, each a unit test with the tolerance stated: linear zero in
      all three components gives `Color::BLACK` and linear one gives
      `Color::WHITE` (exactly); linear 0.5 gives every component within `1e-5`
      of `0.735357`; linear 0.001 gives every component within `1e-6` of
      `0.01292` (the linear segment); `to_linear` of the sRGB grey 0.5 is within
      `1e-5` of `[0.214041, 0.214041, 0.214041, 1.0]`.
- [ ] Round trip: for every 8-bit value `v` in `0..=255`, with
      `c = Color::rgb(v / 255.0, v / 255.0, v / 255.0)`,
      `Color::from_linear_rgba` fed the four values of `c.to_linear()` equals
      `c` within `1e-5` per component, and its `to_rgba8()` equals
      `c.to_rgba8()` exactly.
- [ ] Alpha is not transferred: `from_linear_rgba(0.2, 0.3, 0.4, 0.25).a` is
      exactly `0.25`; `to_linear` of a colour with `a = 0.25` has `0.25` last;
      `from_linear` gives `a = 1.0`.
- [ ] Out of range: `from_linear` of linear -0.5, 2.0 and 0.5 clamps its first
      two components to `0.0` and `1.0` (the third as above). `to_linear` does
      not clamp.
- [ ] No rendered output changes: `git status` after the suite shows no `.png`
      or `.svg` modified.

### AC2: Docs

- [ ] The `Color` doc comment gains a paragraph: a `Color` is sRGB-encoded; a
      host that keeps colours in linear light converts with `from_linear` and
      `to_linear`, for example `theme.background.to_linear()` for a linear clear
      colour. A doctest (external-crate path, `use gup_core::prelude::*`) shows
      `Color::from_linear(0.5, 0.5, 0.5)` and the same colour's `to_linear()`
      round trip within `1e-5`.
- [ ] CHANGELOG `[0.1.0]` Added line.
- [ ] If `docs/EMBEDDING.md` (GUP-702) exists when this lands, add one sentence
      to its texture section pointing at the conversions; if not, skip and say
      so.
- [ ] Search for hand-written transfer functions in the repository's tests,
      examples and docs and report each:

  ```bash
  git grep -nE "0\.04045|0\.0031308|12\.92|powf\(2\.4\)|1\.0 / 2\.4" -- '*.rs' '*.md' \
    ':!docs/planning' ':!docs/archive'
  ```

  Replace a hit with the new methods only where it converts a `Color` in a test
  or example; leave the colour-scale code and the WGSL.

## Decisions

- Signatures, in `impl Color` (not `const`: they use `powf`):

  ```rust
  /// An opaque colour from linear-light components (clamped to 0..=1).
  pub fn from_linear(r: f32, g: f32, b: f32) -> Self;
  /// Like `from_linear`, with straight alpha `a`, which is not transferred.
  pub fn from_linear_rgba(r: f32, g: f32, b: f32, a: f32) -> Self;
  /// The colour as linear-light `[r, g, b, a]`; alpha is unchanged.
  pub fn to_linear(self) -> [f32; 4];
  ```

  `to_linear` returns an array like `to_array` and `to_rgba8`, not a `Color`,
  because a `Color` is sRGB by definition and a linear one in that type would be
  a trap.

- **`from_linear` clamps to `0.0..=1.0`**, as `to_rgba8` and `oklab_to_srgb` do;
  a gup `Color` is documented as in that range, and `powf` of a negative is NaN.
  HDR values are not representable. NaN propagates.
- **The two transfer functions move into `channel.rs`** as `pub(crate)`
  functions of one `f32` (names and bodies unchanged), next to `Color`;
  `scale/sequential.rs` imports them with the line below.

  ```rust
  use crate::channel::{linear_to_srgb, srgb_to_linear};
  ```

  One implementation, so the OKLab colour scales and the public conversion
  cannot disagree.

- **No `Color::from_srgb_u8` or other constructors.** Out of scope.

## Escalation Triggers

- Moving the functions changes a colour-scale golden (it must not: bodies are
  unchanged).
- `srgb_to_linear` is used elsewhere in a way that the move breaks (a path in
  `gup-core-macros` or a test).

## Out of Scope

- Premultiplied or HDR colours, colour spaces other than sRGB and linear sRGB.
- Changing how gup blends (it stays in sRGB space).
- A `Theme` constructor taking linear colours.

## Technical Tasks

- [ ] Move `srgb_to_linear` and `linear_to_srgb` to `channel.rs`; fix imports in
      `sequential.rs`.
- [ ] Add the three methods; unit tests in `channel.rs`'s test module.
- [ ] `Color` doc paragraph and doctest; CHANGELOG; the grep.

## Dependencies

### Prerequisite Stories

- None.

### Enables Stories

- GUP-702: the embedding guide can point at it (soft).

## Testing Strategy

- **Unit tests**: the known values, the 256-value round trip, alpha, clamping.
- **Integration tests**: the existing colour-scale and golden tests pass
  unchanged.
- **Visual validation**: none; no golden changes.
- **Performance**: not applicable.

## Success Metrics

- [ ] A host can make and read back a linear colour without its own transfer
      function.
- [ ] The repository has one sRGB transfer implementation.

## Risk Assessment

- **Low**: pure functions and a move. _Mitigation_: bodies unchanged; goldens
  must not move.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not needed, no golden changes
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm code changes
- [ ] WASM size: `mask wasm-size` scatter gz bytes (three small methods that
      dead-code elimination should drop; report)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
