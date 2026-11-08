---
implementer: sonnet
implementer-reason:
  mostly re-exports, but a wider prelude can collide with names in the
  repository's tests, examples and compile_fail snapshots, which needs a
  judgement call when one does
touches:
  [
    crates/gup-core/src/lib.rs,
    crates/gup-core/tests/compile_fail,
    docs/tour/snippets,
    docs/tutorials,
    CHANGELOG.md,
  ]
---

# GUP-686: The Prelude Holds What Users Reach For

## Story Overview

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: 📋
Ready **Created**: 2026-11-08

## Context

The tour (GUP-669) writes `Color::hex(0x0072b2)` and `Color::hex(0xd55e00)` in
chapters 4 and 6 (`snippets/04_axes_and_labels.rs` lines 102 and 106,
`06_lines_and_curves.rs` lines 37 to 45) because `OKABE_ITO_DISTINCT` (GUP-658)
is exported from the crate root but not from `prelude`. Those hex values are
entries 0 and 1 of that array, so the snippets repeat the default palette by
hand and drift if it changes. The interaction chapters import by path:
`use gup::{Button, Input, Layout}`,
`use gup::{Input, Key, Modifiers, SvgTarget, VectorTarget}`, and tutorial 4 adds
`Event`, `RowId`, `Response`.

`gup_core::prelude` (`crates/gup-core/src/lib.rs`, the `pub mod prelude` block)
already holds `Axes`, `Drag`, `Event`, `Anchor`, `Theme`, `Color`, `Px` and the
scales, but not `Input`, `Key`, `Button`, `Modifiers`, `Response` or `RowId`,
which every interaction snippet needs to drive or answer a chart. The `gup`
crate re-exports `gup_core::prelude` (`src/lib.rs`), so one edit serves both.

`Chart::handle` returns `Response`, which is `#[must_use]` (`input.rs` line
228): a host that drops it never redraws after input, the one mistake the type
exists to prevent. That is why the tour writes `let _ = chart.handle(..)` where
it drives a chart without a window.

## Read First

- `crates/gup-core/src/lib.rs`: the `prelude` module (about line 383) and the
  root re-exports at lines 200 to 220.
- `crates/gup-core/src/input.rs` lines 200 to 235 (`Button`, `Response`).
- `docs/tour/snippets/04_axes_and_labels.rs` lines 95 to 110,
  `06_lines_and_curves.rs` lines 30 to 50, `08_interaction.rs` and
  `09_accessibility.rs` (imports and `handle` calls).
- `crates/gup-core/tests/compile_fail.rs` and one `.stderr` that mentions the
  prelude (`grep -ln prelude crates/gup-core/tests/compile_fail/*.stderr`).

## User Story

> "As a first-time user following the tour, I want the palette and the input
> types in the prelude, so that I do not copy hex codes or hunt for import
> paths."

## Acceptance Criteria

### AC1: The additions

- [x] `prelude` gains exactly: `OKABE_ITO_DISTINCT`, `Input`, `Key`, `Button`,
      `Modifiers`, `Response`, `RowId`. Nothing else is added (reasons under
      Decisions).
- [x] A doctest on the prelude module (or an integration test from an external
      path, `use gup_core::prelude::*` and `use gup::prelude::*`) names each of
      the seven and drives a plot with `Input::pointer_moved`.

### AC2: Collisions

- [x] `cargo check --all-targets` and `cargo check --examples` pass for the
      workspace. Any file that glob-imports the prelude and defines or imports a
      clashing name is fixed by renaming the local item or by an explicit
      import, and listed in the report. (Grep first:
      `grep -rnE "(struct|enum|type|trait|fn) (Key|Input|Button|Modifiers|Response|RowId)\b" crates tests examples docs`.)
- [x] Doctests pass (`cargo test --doc`).

### AC3: The tour and tutorials use them

- [x] `04_axes_and_labels.rs` and `06_lines_and_curves.rs` use
      `OKABE_ITO_DISTINCT[0]`, `[1]`, `[3]` in place of the three hex values
      they match (`0x0072b2`, `0xd55e00`, `0x009e73`; verify each against the
      array). Chapter prose that names a hex code is updated to name the palette
      entry.
- [x] Tour and tutorial imports drop
      `gup::{Button, Input, Key, Modifiers,     Response, RowId}` path imports
      in favour of the prelude where the prelude now covers them. `Layout`,
      `SvgTarget`, `VectorTarget`, `GupApp`, `Closed` and `ScatterChart` stay
      path imports.
- [x] No golden changes: `git status` after the suite shows no `.png` modified
      (the palette values are the same colours). If one does, report it.

### AC4: `handle` and `must_use`

- [x] Decision recorded in the story retrospective and as one sentence in tour
      chapter 9 (where `let _ = chart.handle(..)` first appears): `Response`
      stays `#[must_use]`; headless drivers write `let _ =`. No new API.
- [x] Compile_fail snapshots: run `mask test` (or the compile_fail test with the
      bless variable the file documents). Any `.stderr` that changed because the
      prelude's names appear in a suggestion is re-blessed and the diff read; a
      diff that is anything but a name list is an escalation.

### AC5: CHANGELOG

- [x] `[0.1.0]` Added line naming the seven prelude items.

## Decisions

- **Add the palette constant, not the dark variants**: `OKABE_ITO_DISTINCT` is
  the default, and what a user writes to match or contrast with it.
  `OKABE_ITO_DARK`, `OKABE_ITO_DISTINCT_DARK` and `HIGH_CONTRAST` serve themes,
  which are set by `Theme` constructors; they stay root exports.
- **Add the input vocabulary** (`Input`, `Key`, `Button`, `Modifiers`,
  `Response`, `RowId`): `Event` and `Drag` are already in the prelude and their
  handlers receive `RowId`; any snippet that drives a chart without a window
  needs `Input` and the enums it takes. They are small types with distinctive
  names.
- **Leave out**: `Layout` (read-only result; GUP-685 removes the snippet that
  needed it), `SvgTarget` and `VectorTarget` (target-specific; `save_svg` and
  `to_svg` cover the common case), `GupApp` and `Closed` (feature-gated host),
  `ScatterChart` (already named via the prelude's builder list; the tour's path
  import is for an unusual type position, check and keep if needed), `Brush`,
  `EventInfo` (callback arguments, named by inference). Keeping the prelude
  small is a goal; a user who needs more imports by path.
- **No `Response` ergonomics**: `#[must_use]` is the safety; `let _ =` states
  the deliberate discard and costs six characters. A `handle_headless` or a
  `Response::ignore` is one more name to learn for no safety.
- **Haiku was considered and rejected** because AC2 may need judgement.

## Escalation Triggers

- A prelude name collides with an existing public item re-exported by the
  prelude (not just a test-local type).
- More than five files outside the tour and tutorials need edits for collisions.
- A tour or tutorial golden changes.
- A compile_fail snapshot changes by more than a name list.

## Out of Scope

- Reorganising the root exports or removing prelude items.
- GUP-685's `pixel` and `value` methods (they remove the `Layout` import).
- Changing `Response` or `Chart::handle`.

## Technical Tasks

- [x] Add the seven re-exports; test from an external path.
- [x] Fix collisions found by the grep and `cargo check`.
- [x] Update the tour and tutorial snippets, imports and prose.
- [x] Re-bless compile_fail snapshots if needed; read the diff.
- [x] Chapter 9 sentence; CHANGELOG.

## Dependencies

### Prerequisite Stories

- GUP-658: Default Categorical Order, Distinct First ✅ — `OKABE_ITO_DISTINCT`.
- GUP-669: Feature Tour ✅ — the snippets.

## Testing Strategy

- **Unit tests**: none beyond the doctest and external-path test.
- **Integration tests**: the whole suite, with attention to the compile_fail
  snapshots and tour and tutorial tests.
- **Visual validation**: none; goldens must not change.

## Success Metrics

- [x] The tour contains no `Color::hex` for a default-palette colour.
- [x] Workspace builds with no collision workarounds outside the listed files.

## Risk Assessment

- **Low**: glob-import collisions in downstream user code with their own `Key`
  or `Input`. _Mitigation_: the CHANGELOG line names the additions; a glob
  import is the user's choice.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, no golden changes (AC3)
- [x] `mask wasm-browser`: not needed, re-exports only
- [x] WASM size: unchanged: re-exports only (run `mask wasm-size` to confirm)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `gup_core::prelude` gains `OKABE_ITO_DISTINCT`, `Input`, `Key`, `Button`,
  `Modifiers`, `Response`, `RowId` (`gup::prelude` re-exports it, and so does
  the egui tour). A doctest on the module drives a scatter with them.
- Tour chapters 4 and 6 use `OKABE_ITO_DISTINCT[0]`, `[1]`, `[3]`; chapter 9's
  snippet drops the path import and its page gains the `must_use` sentence;
  tutorial 4 drops its `Button`/`Input`/`Key`/`Response`/`RowId`/`ScatterChart`
  path imports. No golden changed; no compile_fail `.stderr` changed; no
  collision outside the crate's own private `Key`/`Input` types.

## Retrospective

**Completed**: 2026-11-09

### Architectural Decisions

#### `Response` stays `#[must_use]`

- **Decision**: no new API; headless drivers write `let _ =`.
- **Reasoning**: the attribute is the safety; six characters state the intent.
- **Trade-off**: a little noise in tours.

### Development Workflow Insights

- The glob-collision worry did not materialise: the workspace's own `Key` and
  `Input` types are private or in other crates. `ScatterChart` was already in
  the prelude, so its tutorial path import was redundant and removed.
