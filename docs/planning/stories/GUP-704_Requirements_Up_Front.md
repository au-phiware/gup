---
implementer: haiku
implementer-reason:
  docs edits with every fact and its source given, plus one small test whose
  code shape is given
touches:
  [
    README.md,
    docs/tutorials/01_getting_started.md,
    docs/tutorials/README.md,
    docs/RELEASING.md,
    tests/requirements.rs,
    CHANGELOG.md,
  ]
---

# GUP-704: Requirements Up Front

## Story Overview

**Initiative**: Strategic Review 2026-11 (first outside user, before 0.1)
**Status**: ✅ Complete **Created**: 2026-11-09

## Context

The first outside user adopted gup from git at a pinned commit (`d4a66ab4`) into
an app with its own wgpu 27 renderer. What they needed to know first (the Rust
version, which wgpu, what a host's device needs, which features exist, how to
depend on an unpublished crate) was spread over rustdoc and trial and error.

Checked against `main` (2026-11-09):

- **The tutorials state the wrong Rust version.** `docs/tutorials/README.md`
  line 11 says "Rust 1.85+ (edition 2024)" and
  `docs/tutorials/01_getting_started.md` line 23 says "Rust 1.85 or newer, which
  is the first release with edition 2024". The workspace `rust-version` is
  **1.93** (`Cargo.toml` line 20) and `CHANGELOG.md` line 21 says 1.93 for 0.1;
  `rust-toolchain.toml` pins 1.93.1. The README states no Rust version at all.
- **wgpu 27 is real and shows in the host's code.** `Cargo.lock` has `wgpu`
  27.0.1; `gup::wgpu` re-exports it and `lib.rs` line 134 says hosts "must use
  the same major version". wgpu 27's `DeviceDescriptor` has an
  `experimental_features` field, which gup's own device creation sets
  (`context.rs` line 300, `wgpu::ExperimentalFeatures::disabled()`); a host that
  builds the descriptor as a struct literal without `..Default::default()` fails
  to compile without it. This is verified by the code compiling against 27.0.1,
  not by reading wgpu's docs.
- **The README's Quick Start says "Add it from git"** (line 63) with a bare git
  dependency that follows the default branch, so a rebuild can change the
  library under an app. The first user pinned a commit by hand. The tutorials
  and the egui and Bevy guides repeat the bare form.
- **GPU needs are stated only in the tutorials README** as "wgpu-compatible
  GPU". What CI proves is Vulkan on Linux (Mesa's lavapipe software Vulkan on
  the runners, `.github/workflows/README.md` line 81), DX12 on Windows and Metal
  on macOS (README "Key Features"), WebGPU in the browser.
- **`RELEASING.md` step 8** ("switch the docs to the crates.io dependency (the
  README's Quick Start, the tutorials)") does not name the new places this story
  adds, nor the egui and Bevy guides.

## Read First

- `README.md` lines 1 to 80 (Key Features, Not yet, Quick Start).
- `docs/tutorials/01_getting_started.md` lines 19 to 45 (Step 1) and
  `docs/tutorials/README.md` lines 1 to 20.
- `Cargo.toml` lines 1 to 30 and 60 to 80 (`rust-version`, the features:
  `window`, `svg` default; `web`, `arrow` optional).
- `docs/RELEASING.md` step 8 (line 40 on) and `CHANGELOG.md` lines 17 to 24.
- `docs/EGUI_INTEGRATION.md` lines 1 to 20 and `docs/BEVY_INTEGRATION.md` lines
  10 to 25 (their git dependency blocks).
- `.github/workflows/README.md` lines 78 to 86 (what the runners provide).

## User Story

> "As a developer evaluating gup, I want the Rust and wgpu versions, the GPU
> requirement, the features and a way to depend on it before crates.io stated at
> the top of the README and tutorial 1, so that I know what to install before I
> write a line."

## Acceptance Criteria

### AC1: A Requirements section in the README

- [x] `README.md` gains a `## Requirements` section immediately before
      `## Quick Start`, with these statements and no others:
  - Rust 1.93 or newer (read from `Cargo.toml`'s `rust-version`).
  - A GPU or a software driver wgpu supports: Vulkan on Linux (CI uses Mesa's
    lavapipe software driver), DX12 on Windows, Metal on macOS, WebGPU in a
    browser; no GPU crate or async runtime to add.
  - If your app has its own wgpu device, wgpu 27 (`gup::wgpu` is the exact
    version, so `use gup::wgpu`), and that wgpu 27's `DeviceDescriptor` has an
    `experimental_features` field to set (for example
    `wgpu::ExperimentalFeatures::disabled()`) if you build the descriptor
    yourself.
  - The features, as a table taken from `Cargo.toml`: `window` (default), `svg`
    (default), `web`, `arrow`; and that `gup-egui` and `gup-bevy` are separate
    crates.
- [x] The section shows how to depend on gup before crates.io, as two fenced
      `toml` blocks: a git dependency pinned with `rev` (with a placeholder
      commit and a sentence to use the commit you tested against; a branch alone
      follows `main` and can change under you) and a path dependency to a
      checkout (`gup = { path = "../gup" }`). The existing Quick Start sentence
      "Add it from git to `Cargo.toml`" now points at this section instead of
      repeating the block.
- [x] Every fact above is checked against the file named in "Read First" and no
      other fact is added (no unmeasured claims about GPUs or drivers).

### AC2: Tutorial 1 agrees

- [x] `docs/tutorials/01_getting_started.md` Step 1 says Rust 1.93 (drop "the
      first release with edition 2024": true but irrelevant), shows the pinned
      `rev` form once (the placeholder from AC1), and links to the README's
      Requirements section for the rest. Its code blocks are doctests: they
      still compile and the page's tests pass.
- [x] `docs/tutorials/README.md` Prerequisites says Rust 1.93.
- [x] The git blocks in `docs/EGUI_INTEGRATION.md` and
      `docs/BEVY_INTEGRATION.md` gain the same `rev` (one sentence: all gup
      crates must use the same commit).

### AC3: The numbers cannot drift

- [x] A new test `tests/requirements.rs` in the root crate reads the workspace
      `rust-version` from `Cargo.toml` with `toml_edit` (already a
      dev-dependency; parse with `toml_edit::DocumentMut`, read
      `workspace.package.rust-version`), and asserts that `README.md`,
      `docs/tutorials/README.md`, `docs/tutorials/01_getting_started.md` and
      `CHANGELOG.md` each contain that version string, and that none of the
      first three contains `1.85`. It also reads the `wgpu` version from
      `Cargo.lock` (the `[[package]]` named `wgpu`; take its major number) and
      asserts `README.md` contains `wgpu 27` built from that major. Failure
      messages name the file and the expected string.
- [x] The test runs without a GPU and is in `cargo test` (no feature gate).

### AC4: Release checklist and changelog

- [x] `docs/RELEASING.md` step 8 names every place that holds a pre-release
      dependency block: the README's Requirements and Quick Start, tutorial 1,
      `docs/EGUI_INTEGRATION.md`, `docs/BEVY_INTEGRATION.md` and
      `docs/EMBEDDING.md` if it exists (GUP-702), and says to replace the `git`
      and `rev` lines with the crates.io version.
- [x] `CHANGELOG.md` `[0.1.0]` Changed: the tutorials' stated Rust version is
      1.93, and the README has a Requirements section. (If a "Known limitations"
      or "Added" entry already says the same, do not duplicate.)
- [x] `grep -rn "1\.85" README.md docs crates/*/README.md` returns nothing
      outside `docs/planning/`, `docs/archive/` and the CHANGELOG (the command
      is run and its output quoted in the summary).

## Decisions

- **README section, not a new page.** The README is where a stranger lands; the
  facts are five bullets. Tutorial 1 repeats only the version and the dependency
  line and links the rest, so there is one place to update.
- **`rev` is the recommended form** because the first user needed it and a bare
  git dependency moves with `main`. The commit is a placeholder in the docs: a
  real hash would be stale on arrival.
- **The Rust version stated is `rust-version` (1.93)**, not the toolchain pin
  1.93.1, which is the repository's development toolchain.
- **No new GPU claims.** The README's existing claims (Vulkan, DX12, Metal, CI
  on all three) are restated; "software Vulkan" comes from the CI runners'
  setup.
- **The drift test lives in the root crate's tests** so `cargo test` runs it
  with no new tooling; it checks strings, not prose.

## Escalation Triggers

- `rust-version` in `Cargo.toml` and `rust-toolchain.toml` disagree in a way
  that makes "1.93" wrong to state.
- `README.md` already states a wgpu version differently from `Cargo.lock`.
- A tutorial doctest breaks when Step 1 is edited (the page is included as
  doctests from `src/lib.rs`).
- The macOS job is red (RELEASING step 1 forbids claiming macOS support then);
  check the Platforms workflow's latest run before restating "Metal on macOS".

## Out of Scope

- Making gup build on an older Rust.
- The embedding guide (GUP-702) and a host-device limits table.
- Publishing, the crates.io dependency form, `docs.rs` (RELEASING owns those).
- Changing any dependency version.

## Technical Tasks

- [x] README Requirements section; Quick Start pointer.
- [x] Tutorial 1 Step 1 and tutorials README; egui and Bevy guide `rev` lines.
- [x] `tests/requirements.rs`.
- [x] RELEASING step 8; CHANGELOG; the `1.85` grep.

## Dependencies

### Prerequisite Stories

- None.

### Enables Stories

- None.

## Testing Strategy

- **Unit tests**: `tests/requirements.rs` (AC3).
- **Integration tests**: tutorial 1's doctests still pass.
- **Visual validation**: not applicable.
- **Performance**: not applicable.

## Success Metrics

- [x] No page in the repository states a Rust version other than `rust-version`.
- [x] The README names Rust, wgpu, the GPU, the features and a pinned way to
      depend on gup before the Quick Start.

## Risk Assessment

- **Low**: documentation and one string test. _Mitigation_: each fact has a
  named source file.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, no rendering change
- [x] `mask wasm-browser`: not needed, docs and a test only
- [x] WASM size: unchanged: no library code change
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `README.md`: a `## Requirements` section before Quick Start (Rust 1.93, the
  GPU drivers, wgpu 27 with `gup::wgpu` and `experimental_features`, the feature
  table, the separate egui and Bevy crates, and two `toml` blocks: a `rev` pin
  with a placeholder and a path dependency). The Quick Start block now points at
  it.
- `docs/tutorials/01_getting_started.md`: Step 1 says Rust 1.93 and links the
  README; the dependency shows the `rev` placeholder once.
- `docs/tutorials/README.md`: Prerequisites says Rust 1.93+.
- `docs/EGUI_INTEGRATION.md`, `docs/BEVY_INTEGRATION.md`: `rev` on every gup
  line, and one sentence that all gup crates use one commit.
- `docs/RELEASING.md` step 8: names every pre-release dependency block.
- `CHANGELOG.md` `[0.1.0]` Changed: the tutorials' Rust version and the README
  Requirements section.
- `tests/requirements.rs` (new): two tests, no GPU, no feature gate. They read
  `workspace.package.rust-version` (1.93) and the single `wgpu` in `Cargo.lock`
  (27.0.1), and check the README, both tutorial pages and CHANGELOG for the
  version, that none of the first three says `1.85`, and that README says
  `wgpu 27`.

Tests: `tests/requirements.rs` 2 passed. `cargo test -p gup -- --test-threads=1`
passed in every test binary (doctests: 72 and 4 passed, the ignored ones were
already ignored). `cargo check --examples` passed. `mask all-check` exit 0.

Evidence for the `1.85` grep (AC4), run from the worktree root:

```text
$ grep -rn "1\.85" README.md docs crates/*/README.md | grep -v "docs/planning/\|docs/archive/"
(no output, exit 1)
```

Facts checked against the code and files, not against memory:

- `Cargo.toml` `rust-version = "1.93"` (line 20); `rust-toolchain.toml`
  `channel = "1.93.1"`.
- `Cargo.lock`: one `wgpu` package, version 27.0.1.
- `crates/gup-core/src/lib.rs` line 134: "must use the same major version"; line
  135 `pub use wgpu;`.
- `crates/gup-core/src/context.rs` line 300 sets
  `experimental_features: wgpu::ExperimentalFeatures::disabled()`.
  `wgpu-types-27.0.1` (registry source) has `experimental_features` on
  `DeviceDescriptor`, which derives `Default`.
- Features in `Cargo.toml`: `default = ["window", "svg"]`, `web` (wasm32 only,
  per its comment), `arrow`.
- `.github/workflows/README.md` lines 76-88: runners use Mesa's lavapipe.
- Platforms workflow on `main`: latest completed run `success` (checked with
  `gh run list`), which covers the DX12 and Metal jobs.

## Retrospective

**Completed**: 2026-11-09

### Key Technical Learnings

#### Doc facts that drift

- **Challenge**: the tutorials said Rust 1.85 while the manifest said 1.93, and
  nothing failed because the doc text is not compiled.
- **Solution**: `tests/requirements.rs` reads the version from `Cargo.toml` and
  the wgpu major from `Cargo.lock`, and checks the strings in the docs.
- **Pattern**: a string check against the manifest is cheap and needs no GPU. It
  catches a stale version, not wrong prose.

#### Tutorial pages are doctests

- **Challenge**: `src/lib.rs` includes the README and every tutorial as
  doctests, so an edit to prose or a code block is compiled.
- **Solution**: only prose and `toml` blocks changed in tutorial 1, and the
  doctest run (72 and 4 passed) confirmed it.

### Architectural Decisions

#### `rev` pin as the recommended git form

- **Decision**: the docs show `rev = "<full commit hash>"` and a path
  dependency, and say a branch alone is not enough.
- **Reasoning**: a bare git dependency follows `main`, so a rebuild can change
  the library under an app. The first outside user pinned a commit by hand.
- **Trade-off**: the placeholder must be replaced by the reader. A real hash
  would be stale on arrival.
- **Future**: RELEASING step 8 names each block to switch to crates.io.

### Development Workflow Insights

- `mask all-fix` runs prettier across every markdown file. It changed nothing
  outside this story's files, but check `git status` after it.
- Worktree-isolated shells refuse some plain commands (variables in `sed`, `git`
  in a `python` heredoc). Plain commands with literal paths work.

### Observed, not changed (outside this story's ACs)

- `README.md` "Not yet" says "No CSV, serde or Arrow input", and `CHANGELOG.md`
  `[0.1.0]` Known limitations says the same, but `Cargo.toml` has an `arrow`
  feature (`Columns::from_arrow`). The Requirements table, which this story
  added, lists `arrow`, so the README now contradicts itself. The fix is one
  line in each place. It is not in this story's ACs, so it is reported rather
  than edited.

### Follow-up Stories

1. **Arrow wording in "Not yet" and Known limitations**: a one-line correction
   in `README.md` and `CHANGELOG.md`, to say Arrow record batches are optional
   (feature `arrow`), not absent. Small enough to fold into the next docs pass.
