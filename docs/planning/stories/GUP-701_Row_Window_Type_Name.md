---
implementer: sonnet
implementer-reason:
  the new name is decided below; the work is a sweep in which the compiler finds
  the code and a grep finds the prose, with the care point that winit's `Window`
  must not be renamed
touches:
  [
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/lib.rs,
    crates/gup-core/src/builder,
    docs/tour,
    docs/tutorials,
    docs/TECHNICAL_APPROACH.md,
    gup-egui/examples/tour.rs,
    CHANGELOG.md,
  ]
---

# GUP-701: The Row Window Type No Longer Collides With winit's `Window`

## Story Overview

**Initiative**: Strategic Review 2026-11 (first outside user, before 0.1)
**Status**: ✅ Complete 2026-11-11 **Created**: 2026-11-09

## Context

The first outside user's app is a winit host. Every module that draws gup charts
into the app's own renderer imports `winit::window::Window` and also meets
`gup::Window` (the row-window enum, `Window::All` and `Window::LastRows(n)`,
`selection.rs` line 170), which is in `gup::prelude` (`lib.rs` line 415) and at
the root (line 224). The two names mean different things: one is the OS window,
the other is how many rows a streaming layer keeps.

Checked against `main` (2026-11-09), the report overstates how it fails:

- An explicit `use winit::window::Window;` shadows the prelude's glob import, so
  a module that imports winit's `Window` compiles and gets winit's. The
  collision is not a compile error there. It bites when a module globs both
  (`use winit::window::*;` with `use gup::prelude::*;`: an ambiguity error on
  first use), when it uses `Window` without importing winit's (the error says
  `expected winit::Window, found gup::Window`, or the reverse), and in the
  reader's head: `.window(Window::LastRows(2_000))` next to a winit window.
- gup's own `crate::window` module (`WindowTarget`, `window.rs`) and its
  `winit::window::Window` imports (`window.rs`, `app.rs`, `gpu_pick.rs`) are
  winit's, not the row window, and must not be renamed.
- `Retain` (`selection.rs` line 105) is taken: it says what the CPU keeps of a
  chunk (`Auto`, `GpuOnly`, ...). `Keep` would sit beside it and mean something
  else, so it is rejected.
- 0.1 is unpublished, so the rename needs no shim.

## Read First

- `crates/gup-core/src/selection.rs`: lines 100 to 200 (`Retain`, `Window`, its
  `impl`, the doctest at 150 to 168), line 639 to 660 (`Selection::window`).
- `crates/gup-core/src/lib.rs`: line 224 (root re-export), line 415 (prelude).
- `crates/gup-core/src/builder/line.rs` line 244 and `builder/scatter.rs` line
  255 (`.window(..)`).
- `docs/tutorials/05_streaming_data.md` and `docs/tour/10_live_data.md` (where
  the type is taught).
- The list of sites, from

  ```bash
  git grep -nw "Window" -- '*.rs' '*.md' ':!docs/planning' ':!docs/archive'
  ```

  where every hit is either the row window (rename) or winit's, the OS window or
  the word "window" in prose (leave).

## User Story

> "As a developer who hosts gup in a winit app, I want the type that bounds a
> streaming layer's rows to have a name that is not winit's `Window`, so that
> importing both never needs an alias or a second look."

## Acceptance Criteria

### AC1: The type is `RowWindow`

- [x] `gup_core::Window` is renamed `RowWindow` (enum, `impl`, docs, doctests),
      with the variants `All` and `LastRows(usize)` unchanged. Re-exported at
      the root and in the prelude as `RowWindow`; nothing at the root or in the
      prelude is named `Window`.
- [x] The methods keep their names and take the new type:
      `Selection::window(&mut self, window: RowWindow)`, `LineChart::window`,
      `ScatterChart::window` (and any other builder found by
      `grep -rn "fn window" crates/gup-core/src/builder`).
- [x] An external-crate doctest (the prelude doc block in `lib.rs`) compiles
      `use gup::prelude::*; use winit::window::Window;` together with
      `gup::line(rows).window(RowWindow::LastRows(2_000))`, with no alias and no
      ambiguity. (`winit` is a dependency of `gup` under feature `window`, the
      default; put the doctest where that feature is on.)

### AC2: Every site is swept

- [x] `cargo check --workspace --all-targets` and `cargo test --doc` compile.
      Sites that need the new name include `selection.rs` (23 hits),
      `builder/line.rs`, `builder/scatter.rs`, `lib.rs`,
      `examples/zoom_bench.rs`, `tests/window.rs`, `tests/window_parity.rs`,
      `dogfood/src/bin/t5_live.rs`, `gup-egui/examples/tour.rs`,
      `docs/tour/10_live_data.md` and its snippet
      `docs/tour/snippets/10_live_data.rs`,
      `docs/tutorials/05_streaming_data.md`, `docs/tutorials/README.md`,
      tutorial 1, `README.md`, `docs/TECHNICAL_APPROACH.md`,
      `docs/IMPLEMENTATION_STRATEGY.md`, `docs/ACCESSIBILITY.md`,
      `CONVENTIONS.md` where each means the row window. The list is a start; the
      grep is the authority.
- [x] winit's `Window` is untouched: `git diff` shows no change in `window.rs`,
      `app.rs`, `gpu_pick.rs`, `web.rs`, `gup-bevy`, `CLAUDE.md` or
      `.github/agents/screen-grabber.md` (the last three mention the OS window).
- [x] The tour page `docs/tour/10_live_data.md` matches its snippet
      (`pages_match_their_snippets` passes); no tour or tutorial golden changes.
- [x] CHANGELOG `[0.1.0]` Changed: `Window` is now `RowWindow`.
- [x] After the sweep this returns only winit's or prose:

  ```bash
  git grep -nE "\bWindow::(All|LastRows)|window\(Window" -- '*.rs' '*.md' \
    ':!docs/planning' ':!docs/archive'
  ```

## Decisions

- **The name is `RowWindow`.** It says what is windowed (rows), it is one word
  that cannot be mistaken for the OS window, and the existing `LastRows` variant
  reads redundantly but correctly (`RowWindow::LastRows(n)`). Rejected: `Keep`
  (beside `Retain`, which says something else), `Retain` (taken), `Rolling` and
  `Tail` (suggest an aggregate or a position, not a bound), `History` (suggests
  undo).
- **The methods are still `.window(..)`.** A method name cannot collide with a
  type in the host's imports, `window` is the established word in the docs and
  tutorials, and renaming both would double the sweep for no gain.
- **No alias and no shim.** 0.1 is unpublished.
- **`crates/gup-core/src/window.rs` and `WindowTarget` stay.** They are the OS
  window's target.
- **Test file names stay** (`tests/window.rs`, `window_parity.rs`): they test
  the row window's feature, not the type's spelling.

## Escalation Triggers

- A hit that could be either meaning (prose "the window" in a doc that talks
  about both): keep the prose and say which you decided.
- The doctest in AC1 cannot enable `winit` (feature off in the doctest build).
- A pinned message or snapshot contains `Window::LastRows`.
- `Window` is used in a derive or macro expansion in `gup-core-macros`.

## Out of Scope

- Renaming `Retain` or the `.window` methods.
- Changing the eviction rule or chunk sizes.
- GUP-700's `Point` rename, which edits some of the same docs (see run order in
  the index).

## Technical Tasks

- [x] Rename the enum in `selection.rs` and fix its docs and doctests.
- [x] Root and prelude re-exports; builder signatures.
- [x] Sweep code, tour, tutorials and docs by the grep, judging each hit.
- [x] The two-import doctest; CHANGELOG.

## Dependencies

### Prerequisite Stories

- GUP-686: The Prelude Holds What Users Reach For ✅ — the prelude's contents
  are settled enough to name against.

### Enables Stories

- GUP-702: the embedding guide's winit examples import both without an alias.

## Testing Strategy

- **Unit tests**: `tests/window.rs` and `window_parity.rs` pass renamed.
- **Integration tests**: tour and tutorial tests; `cargo check --examples`.
- **Visual validation**: none; no golden changes.
- **Performance**: not applicable.

## Success Metrics

- [x] A winit host can `use winit::window::Window; use gup::prelude::*;` and use
      both names with no alias.

## Risk Assessment

- **Low**: a rename inside an unpublished crate. _Mitigation_: the compiler
  finds code; the grep finds prose.
- **Medium**: renaming winit's `Window` by mistake in a docs file. _Mitigation_:
  the unchanged-files AC.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, no golden changes
- [x] `mask wasm-browser`: not needed, a rename with no behaviour change
- [x] WASM size: unchanged: no library code change beyond names
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

`Window` is now `RowWindow` (enum, `impl`, docs, doctests), re-exported at the
root and in the prelude; `Selection::window`, `LineChart::window` and
`ScatterChart::window` take it and keep their names. Swept: `selection.rs`,
`builder/line.rs`, `builder/scatter.rs`, `gpu_pick.rs` (a doc link), `lib.rs`,
`tests/window.rs`, `tests/live_view.rs`, `dogfood/src/bin/t5_live.rs`,
`gup-egui/examples/tour.rs`, the tour chapter 10 page and snippet, tutorial 5
and the tutorials README, `README.md`, `docs/TECHNICAL_APPROACH.md`,
`docs/IMPLEMENTATION_STRATEGY.md` and `CHANGELOG.md` (a new Changed line and the
streaming line). winit's `Window` (`window.rs`, `app.rs`, `zoom_bench.rs`,
gup-bevy, `CLAUDE.md`) is untouched. No compile_fail snapshot or golden
contained the name, so nothing was re-blessed.

New tests: `tests/row_window.rs` (a host's own `Window` beside the prelude; one
type at root and prelude) and a prelude doctest that imports
`winit::window::Window` with `gup_core::prelude::*` and calls
`.window(RowWindow::LastRows(2_000))`, gated on feature `window`.

## Retrospective

**Completed**: 2026-11-11

### Key Technical Learnings

#### A rename sweep by use shape

- **Challenge**: `Window` is also winit's, `WindowTarget`'s and prose.
- **Solution**: sed on the specific shapes (`Window::All|LastRows`,
  `window: Window`, a linked `Window`) in a named file list; the story's grep
  then returned nothing.
- **Pattern**: never sed a bare word across the tree.

#### Doctests can cover optional features

- **Solution**: a gup-core doctest gated with `# #[cfg(feature = "window")]` can
  use winit, which the facade's tests cannot (not a dev-dependency).

### Architectural Decisions

#### The two-import check is a gup-core doctest

- **Decision**: the check lives in gup-core's prelude docs.
- **Reasoning**: winit is gup-core's optional dependency, not the facade's.
- **Trade-off**: it does not run through the `gup` path; `tests/row_window.rs`
  covers that with a local `Window` type.

### Development Workflow Insights

- A doc link added in a doctest paragraph failed the `docs-rs` step of
  `mask merge-check` (`redundant_explicit_links`); `mask all-fix` does not catch
  it.

### Follow-up Stories

None.
