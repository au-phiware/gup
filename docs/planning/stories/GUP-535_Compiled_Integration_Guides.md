---
implementer: sonnet
implementer-reason:
  two guides made doctests of the crates they describe, with the include pattern
  and block tags fixed; may need small fixes where a snippet is stale
touches: []
---

# GUP-535: Compile the egui and Bevy Integration Guides

## Story Overview

**Initiative**: Strategic Review 2026-11 (N1 Docs and Tutorials) **Status**: 🚧
In Progress Planned **Created**: 2026-10-24

## Context

`docs/EGUI_INTEGRATION.md` and `docs/BEVY_INTEGRATION.md` were rewritten for
`gup-core` in GUP-483 and GUP-484, but their code blocks are `rust,ignore`, so
nothing checks them. They are the pages a host-application developer copies
from, and the next API change (track N2 renames or adds builder methods) can
break them silently. The tutorials and architecture docs are doctests of the
`gup` crate (GUP-489, GUP-530); these two guides belong to the crates they
describe, whose dependencies (eframe, bevy) the `gup` crate does not have.

## Read First

- `docs/EGUI_INTEGRATION.md` and `docs/BEVY_INTEGRATION.md` (whole files).
- `gup-egui/src/lib.rs` lines 1–40 and `gup-egui/Cargo.toml`.
- `gup-bevy/src/lib.rs` lines 1–40 and `gup-bevy/Cargo.toml`.
- `gup-egui/examples/egui_chart.rs` and `gup-bevy/examples/bevy_scatter.rs`
  (working programs to fix stale snippets against).
- `src/lib.rs` lines 78–86 (the include pattern).
- `.github/workflows/tests.yml` lines 110–150 (the `integrations` job runs
  `cargo test -p gup-bevy`).

## User Story

> "As a developer embedding Gup in an egui or Bevy app, I want the guide's code
> to be checked by CI, so that what I copy compiles."

## Acceptance Criteria

### AC1: The egui guide is a doctest of `gup-egui`

- [x] `gup-egui/src/lib.rs` gains
      `#[cfg(doctest)] #[doc = include_str!("../../docs/EGUI_INTEGRATION.md")] struct EguiGuide;`.
- [x] Every Rust block in the guide is `rust,no_run` (it opens a window) and is
      a complete program with `fn main() -> eframe::Result`, built like
      `examples/egui_chart.rs`'s `main`. No `rust,ignore`, no hidden
      `#`-prefixed lines.
- [x] `cargo test -p gup-egui --doc` passes and reports the guide's blocks.
- [x] The `toml` block's paths are replaced with git dependencies
      (`gup-egui = { git = "https://github.com/au-phiware/gup" }`), matching
      GUP-530's dependency decision, and `eframe` keeps its version from
      `gup-egui/Cargo.toml`.

### AC2: The Bevy guide is a doctest of `gup-bevy`

- [x] `gup-bevy/src/lib.rs` gains the same include for
      `docs/BEVY_INTEGRATION.md` (`struct BevyGuide`).
- [x] Its Rust blocks are `rust,no_run` complete programs; its `toml` block uses
      git dependencies and the `bevy` features from `gup-bevy/Cargo.toml`.
- [x] `cargo test -p gup-bevy --doc` passes locally (it is run in CI by the
      `integrations` job's `cargo test -p gup-bevy`; no workflow change).

### AC3: Proof that the guides are checked

- [x] Seeded proof, recorded and not committed: renaming `GupWidget::show` to
      `GupWidget::shown` in the egui guide makes `cargo test -p gup-egui --doc`
      fail.

## Decisions

- **Doctests of the integration crates**, not of `gup`: they need `eframe` and
  `bevy`, which only those crates depend on.
- **`no_run`**: a guide's program opens a window; compiling it is the check.
  Rendering is already covered by `gup-egui`'s screenshot parity test and
  `gup-bevy`'s tests.
- **Fix, don't rewrite**: if a snippet no longer compiles, change it to the
  smallest working form taken from the crate's example; prose changes only where
  the code changed.

## Escalation Triggers

- `cargo test -p gup-bevy --doc` takes more than 10 minutes on a warm build
  directory: report the time; do not drop the Bevy guide silently.
- A guide describes behaviour the crate no longer has (not just a renamed call):
  report it.

## Out of Scope

- New integration features; the egui and Bevy examples themselves.
- Moving `gup-bevy` into `mask all-check` (its build cost keeps it in the
  `integrations` job).

## Technical Tasks

- [x] Start commit within 30 minutes.
- [x] egui: retag, complete the program, include, run `--doc`; commit.
- [x] Bevy: the same; commit.
- [x] Seeded proof; DoD commands.

## Dependencies

### Prerequisite Stories

- GUP-483: `gup-egui` on `gup-core` ✅; GUP-484: `gup-bevy` on `gup-core` ✅.

### Enables Stories

- None.

## Testing Strategy

- **Doctests**: `cargo test -p gup-egui --doc`, `cargo test -p gup-bevy --doc`.

## Success Metrics

- [x] No `rust,ignore` block remains in `docs/*.md`:
      `git grep -n 'rust,ignore' -- docs/*.md` prints nothing.

## Risk Assessment

- **Low**: Bevy doctests are slow to link. _Mitigation_: one or two blocks only;
  `no_run` skips execution.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-egui -- --test-threads=1` and
      `cargo test -p gup-bevy -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output: not applicable, no visual change
- [x] `mask wasm-browser`: not needed, no layout, input or wasm change
- [x] WASM size: unchanged, no library code change
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

Completed 2026-10-25. `gup-egui` and `gup-bevy` include their guides as
`#[cfg(doctest)]` doctests (`EguiGuide`, `BevyGuide`). All Rust blocks are
`rust,no_run`; the egui block gained a `fn main() -> eframe::Result`; toml
blocks use git dependencies (egui adds `egui`, Bevy adds the `png` feature from
`gup-bevy/Cargo.toml`). `cargo test -p gup-egui --doc` reports 2 passed,
`cargo test -p gup-bevy --doc` 3 passed (31 s warm).

Seeded proof (not committed): `show` renamed to `shown` in the egui guide gave
`error[E0599]: no method named shown found for struct GupWidget` and
`test gup-egui/src/lib.rs - EguiGuide (line 75) - compile ... FAILED`.

## Retrospective

**Completed**: 2026-10-25

### Key Technical Learnings

#### Guides as crate doctests

- **Challenge**: the guides need eframe and bevy, which `gup` lacks.
- **Solution**: include them in the crates that depend on those.
- **Pattern**: the Bevy guide was already a complete program; only the egui one
  needed a `main`. The guides cost ~30 s of doctest time warm.

### Follow-up Stories

None.
