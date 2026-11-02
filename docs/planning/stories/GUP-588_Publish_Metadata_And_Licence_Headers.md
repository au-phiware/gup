---
implementer: sonnet
implementer-reason:
  manifest edits whose every field and value is fixed in Decisions, plus one
  small shell check; every AC is a command
touches:
  [
    Cargo.toml,
    crates/gup-core/Cargo.toml,
    crates/gup-core-macros/Cargo.toml,
    crates/gup-text/Cargo.toml,
    crates/gup-wgsl/Cargo.toml,
    crates/gup-visual-regression/Cargo.toml,
    gup-egui/Cargo.toml,
    gup-bevy/Cargo.toml,
    Cargo.lock,
    maskfile.md,
    crates/gup-core/wasm-size/scatter/index.html,
  ]
---

# GUP-588: Publish Metadata and Licence Headers

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 🚧 In
Progress **Created**: 2026-11-02

## Context

Release 0.1 publishes Gup to crates.io (owner decision 2, 2026-10-24). Today
nothing can be packaged. `cargo package --workspace --no-verify` stops at the
first internal dependency: "all dependencies must have a version requirement
specified when packaging. dependency `gup-wgsl` does not specify a version"
(checked 2026-11-02). `gup-core`, `gup-core-macros`, `gup-text` and `gup-wgsl`
are `publish = false`. Three of their descriptions are written for this
project's own planning documents, not for a crates.io reader. Examples:
"`#[wgsl_function]` (RFC-001 §6, GUP-420)" and "Gup's text system (RFC-001 S2)".
The root package would also ship `docs/` (13 MB, the planning history included),
`scripts/`, `.github/` and the flake, because it is the workspace root and has
no `include` list.

The licence stays GPL-3.0-or-later (owner decision 1). Every manifest already
says so. Every code file has the two-line header except `dogfood/src/main.rs` (a
one-line `fn main() {}`), `crates/gup-core/wasm-size/scatter/index.html` and the
generated `docs/gallery/index.html`. Nothing checks the headers, and no package
ships the licence text.

The crate names are free. On 2026-11-02,
`https://crates.io/api/v1/crates/<name>` returned 404 for `gup`, `gup-core`,
`gup-core-macros`, `gup-wgsl`, `gup-text`, `gup-egui` and `gup-bevy`. Claiming
them is an owner step (see INDEX.md, N3).

## Read First

- `Cargo.toml` (the root: `[workspace]`, `[package]`, `[[example]]`,
  `[package.metadata.docs.rs]`).
- Each member's `[package]` and `[dependencies]` block:
  `crates/gup-core/Cargo.toml` lines 1–30, `crates/gup-core-macros/Cargo.toml`,
  `crates/gup-text/Cargo.toml` (note its `include`),
  `crates/gup-wgsl/Cargo.toml`, `gup-egui/Cargo.toml`, `gup-bevy/Cargo.toml`.
- `maskfile.md` "## all-check" (lines 284–342): where the new check goes.
- `scripts/check_story_ids.sh`: the shape of a small repository check.

## User Story

> "As a Rust developer, I want `cargo add gup` to give me a crate with an
> accurate description, a licence and only the files it needs, so that I can
> judge and adopt it from crates.io."

## Acceptance Criteria

### AC1: Shared metadata

- [ ] The root `Cargo.toml` has a `[workspace.package]` with exactly the values
      in Decisions. Every member listed in Decisions inherits `version`,
      `authors`, `edition`, `license`, `repository`, `homepage` and
      `rust-version` through `<field>.workspace = true`.
- [ ] `[workspace.dependencies]` declares the six internal library crates with
      `path` and `version = "=0.1.0"`. Every internal dependency in every
      member's `[dependencies]` and `[build-dependencies]` uses
      `{ workspace = true }`, with its features where it has them today. So does
      every `[dev-dependencies]` entry naming one of the six.
      `gup-visual-regression` stays a path-only dev-dependency, which
      `cargo package` strips.
- [ ] `publish = false` is removed from `gup-core`, `gup-core-macros`,
      `gup-text` and `gup-wgsl`. `gup-visual-regression` keeps it.
- [ ] `cargo metadata --format-version 1 --no-deps | jq -r '.packages[] | select(.publish == null) | .name' | sort`
      prints exactly the seven published crates of Decisions.

### AC2: Descriptions, keywords, categories, READMEs

- [ ] Each published crate's `description`, `keywords` and `categories` are
      exactly those in the Decisions table.
      `grep -n 'RFC\|GUP-' */Cargo.toml crates/*/Cargo.toml Cargo.toml | grep -E '^[^:]+:[0-9]+:(description|keywords)'`
      prints nothing.
- [ ] `crates/gup-core`, `crates/gup-core-macros`, `crates/gup-text` and
      `crates/gup-wgsl` each have a `README.md` with the text of Decisions, and
      `readme = "README.md"`. `gup-text`'s `include` lists `README.md` and
      `COPYING`.

### AC3: Licence text and headers

- [ ] Each published crate's directory has `COPYING`, a relative symlink to the
      root `COPYING`: `../COPYING` for `gup-egui` and `gup-bevy`, and
      `../../COPYING` for the four under `crates/`.
      `cargo package --list -p <crate> --allow-dirty` lists `COPYING` for all
      seven.
- [ ] `scripts/check_headers.sh` exits 1 and names each git-tracked file that
      lacks `SPDX-License-Identifier: GPL-3.0-or-later` in its first 5 lines. It
      checks files matching `*.rs *.wgsl *.sh *.mjs *.js *.css *.html`, except
      the exemptions in Decisions. It exits 0 on this tree after the three fixes
      below. A seeded run proves it: a scratch copy of one `.rs` file with its
      header removed, passed as an argument, makes it exit 1.
- [ ] `dogfood/src/main.rs` and `crates/gup-core/wasm-size/scatter/index.html`
      have the header. `scripts/generate_gallery_html.sh` writes the header as
      an HTML comment on the generated page's second line, and the regenerated
      `docs/gallery/index.html` has it. The comment's text is in Decisions.
- [ ] `mask all-check` runs `./scripts/check_headers.sh` in its `gallery` group
      (after `check_story_ids.sh`), and the all-check prose mentions it in one
      sentence.

### AC4: What the root package ships

- [ ] The root `[package]` has the `include` list of Decisions.
      `cargo package --list -p gup --allow-dirty` lists no path under `docs/`
      other than the three included documents and `docs/tutorials/*.md`, and
      nothing under `scripts/`, `.github/`, `tests/`, `dogfood/` or
      `docs/planning/`.
- [ ] `cargo package --workspace --allow-dirty --no-verify` succeeds. Each
      `.crate` file it writes under `$CARGO_TARGET_DIR/package/` is under 5 MB:
      `ls -l` its output and paste the sizes into the Implementation Summary.

### AC5: Nothing else changes

- [ ] `mask all-check` passes, and `cargo test -- --test-threads=1` at the root
      passes. `Cargo.lock` changes only in the internal crates' entries, if at
      all: `git diff Cargo.lock` shows no external version change.
- [ ] `git diff --stat` touches only the files in this story's `touches` list,
      the four new READMEs, the six `COPYING` symlinks,
      `scripts/check_headers.sh`, `scripts/generate_gallery_html.sh`,
      `docs/gallery/index.html` and `dogfood/src/main.rs`.

## Decisions

- **Workspace package values** (root `Cargo.toml`):

  ```toml
  [workspace.package]
  version = "0.1.0"
  authors = ["Corin Lawson <corin@phiware.com.au>"]
  edition = "2024"
  license = "GPL-3.0-or-later"
  repository = "https://github.com/au-phiware/gup"
  homepage = "https://github.com/au-phiware/gup"
  rust-version = "1.93"
  ```

  `rust-version` is the pinned toolchain's minor version, the only one CI tests.
  GUP-596's semver policy says how it may move. `gup-visual-regression` inherits
  the same fields; its `publish = false` stays.

- **Published crates, and lockstep versions.** These seven are published:
  `gup-wgsl`, `gup-text`, `gup-core-macros`, `gup-core`, `gup`, `gup-egui` and
  `gup-bevy`. The order is the dependency order; `cargo publish --workspace`
  computes it. Internal requirements are exact (`=0.1.0`) because the
  macro-generated code and the WGSL library text are an internal contract
  between `gup-core-macros`, `gup-wgsl` and `gup-core`. A `gup-core` 0.1.1 must
  never resolve against a `gup-wgsl` 0.1.0 it was not built with. Every release
  bumps all seven together. Example of the workspace entries:

  ```toml
  [workspace.dependencies]
  gup-core = { path = "crates/gup-core", version = "=0.1.0" }
  gup-core-macros = { path = "crates/gup-core-macros", version = "=0.1.0" }
  gup-wgsl = { path = "crates/gup-wgsl", version = "=0.1.0" }
  gup-text = { path = "crates/gup-text", version = "=0.1.0" }
  ```

  `gup` is never a dependency of another member, and `gup-egui` and `gup-bevy`
  depend on nothing internal but `gup-core`, so those four entries are all. A
  member with features writes, for example,
  `gup-wgsl = { workspace = true, features = ["compose"] }`.

- **Descriptions, keywords and categories** (crates.io allows at most five
  keywords; every category is a valid crates.io slug):
  - `gup`: description
    `Charts drawn on the GPU: scatter, line, bar, area, histogram, heatmap and box plots in a window, PNG, SVG, egui, Bevy or the browser`;
    keywords `["chart", "plot", "visualization", "gpu", "wgpu"]`; categories
    `["visualization", "graphics", "wasm"]`.
  - `gup-core`: description
    `The engine of the gup charting library: a GPU column store, scales, marks, layout and render targets on wgpu`;
    keywords `["chart", "plot", "visualization", "gpu", "wgpu"]`; categories
    `["visualization", "graphics", "rendering"]`.
  - `gup-core-macros`: description
    `Procedural macros for gup-core: #[wgsl_function] and #[derive(Mark)]`;
    keywords `["chart", "wgsl", "shader", "macro"]`; categories
    `["visualization", "development-tools::procedural-macro-helpers"]`.
  - `gup-wgsl`: description
    `The WGSL shader library of the gup charting library, with build-time composition and a run-time linker`;
    keywords `["wgsl", "shader", "webgpu", "wgpu"]`; categories
    `["graphics", "rendering"]`.
  - `gup-text`: description
    `Text measurement and GPU glyph drawing for the gup charting library, with the Inter font bundled`;
    keywords `["text", "font", "glyph", "wgpu"]`; categories
    `["graphics", "rendering", "text-processing"]`.
  - `gup-egui`: description `An egui widget that draws gup charts`; keywords
    `["chart", "plot", "visualization", "egui", "gpu"]`; categories
    `["visualization", "gui"]`.
  - `gup-bevy`: description `A Bevy plugin that draws gup charts`; keywords
    `["chart", "plot", "visualization", "bevy", "gpu"]`; categories
    `["visualization", "game-engines"]`.

  `gup`'s `readme = "README.md"` stays. The Cargo comments that cite RFC-001 or
  GUP numbers stay: they are for contributors, and the published
  `Cargo.toml.orig` carries them harmlessly.

- **The four new crate READMEs** are the same text with the crate's own name:

  ```markdown
  # <crate>

  Part of [Gup](https://github.com/au-phiware/gup), GPU-accelerated charts for
  Rust. <one sentence: the crate's description from the table above>.

  Most users want the [`gup`](https://crates.io/crates/gup) crate, which
  re-exports everything this crate provides to applications. Licensed under
  GPL-3.0-or-later.
  ```

  For `gup-wgsl`, `gup-text` and `gup-core-macros`, put "Applications depend on
  `gup`; this crate is an implementation detail of it." in place of the sentence
  starting "Most users want".

- **Licence text by symlink**, not by copy: one source, so no drift. Cargo
  packages a symlinked file's contents. On a Windows checkout without symlink
  support, the link becomes a one-line text file. Nothing packages on Windows,
  so that is harmless.

- **Header check exemptions**: `crates/gup-core/tests/fixtures/*.wgsl`
  (generated glue the tests compare byte for byte) and `crates/gup-culling-lod/`
  (parked, frozen). Everything else is fixed, not exempted. The generated
  gallery page gets the header from its generator, as this second line:
  `<!-- Copyright (C) 2024 Corin Lawson. SPDX-License-Identifier: GPL-3.0-or-later -->`.
  The check reads `git ls-files` (tracked files only) and also accepts paths as
  arguments, which is how the seeded run tests it. It is a plain bash script
  with the two-line header, and it passes `shellcheck`.

- **Root `include`** (the facade's source, its examples, and the documents its
  `cfg(doctest)` modules include, so `cargo test` works from the tarball):

  ```toml
  include = [
    "src/**",
    "examples/**",
    "README.md",
    "COPYING",
    "Cargo.toml",
    "docs/TECHNICAL_APPROACH.md",
    "docs/IMPLEMENTATION_STRATEGY.md",
    "docs/tutorials/*.md",
  ]
  ```

  `tests/` is not shipped: the goldens are 2.3 MB and the integration tests need
  `gup-visual-regression`, which is not published. Cargo drops the
  auto-discovered test targets from the packaged manifest.

- **Out of the packages**: `examples/README.md` and `examples/INDEX.md` ship
  with `examples/**`. That is fine: they are small, and they link to the
  repository.

## Escalation Triggers

- `cargo package --workspace --no-verify` fails for a reason other than
  metadata: for example, a build script reads a file outside its package.
  `gup-core`'s `build.rs` reads `gup_wgsl::compose::library()`, which `gup-wgsl`
  embeds from its own `library/` directory at its own build time. If that turns
  out to read `../gup-wgsl/library` through the file system, stop and report.
- Any crate's `.crate` file is 5 MB or more.
- Inheriting `rust-version` makes clippy or rustc warn under `-D warnings` (for
  example, `incompatible_msrv` on an API newer than 1.93). Report the lint and
  the call site; do not lower or raise the pin.
- The `=0.1.0` requirements make `cargo metadata` or `cargo tree` resolve two
  copies of an internal crate.

## Out of Scope

- `cargo publish --dry-run` with verification, and the docs.rs build, in CI:
  GUP-589.
- `[package.metadata.docs.rs]` for crates other than `gup`: GUP-589.
- CHANGELOG and the semver policy: GUP-596.
- The README's text, hero image and install instructions: GUP-597 and GUP-599.
- Publishing, and claiming the names: owner steps.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30
      minutes.
- [ ] `[workspace.package]` and `[workspace.dependencies]`; convert each member;
      `cargo metadata` check (AC1). Commit.
- [ ] Descriptions, keywords, categories, READMEs, `include` (AC2, AC4). Commit.
- [ ] COPYING symlinks, `check_headers.sh`, the three header fixes, the
      generator change and the regenerated page, the all-check entry (AC3).
      Commit.
- [ ] `cargo package --workspace --allow-dirty --no-verify`; sizes; DoD
      commands.

## Dependencies

### Prerequisite Stories

- GUP-498: Current gup-core crate docs ✅ — the crate docs are already free of
  RFC jargon; this story does the manifests.
- GUP-564: Derive role message and stale gallery ✅ — the gallery generator and
  link check this story's header change runs through.

### Enables Stories

- GUP-589: Publish dry run and docs.rs build in CI — needs packageable
  manifests.
- GUP-598: Release checklist.

## Testing Strategy

- **Commands**: `cargo metadata`, `cargo package --list`,
  `cargo package --workspace --no-verify`, `scripts/check_headers.sh` (seeded
  and clean).
- **Regression**: `mask all-check`, root `cargo test -- --test-threads=1`.
- **Visual validation**: none; nothing renders differently.

## Success Metrics

- [ ] Seven crates package without error; every `.crate` file is under 5 MB.
- [ ] One header check guards every code file in CI and locally.

## Risk Assessment

- **Low**: workspace inheritance changes how `cargo` reports the packages, and a
  script reads a manifest field directly. _Mitigation_: grep `scripts/` and
  `dogfood/` for `version =` and `description` reads before converting.
  `check_gallery_sync.sh` reads `[[example]]` only.
- **Low**: crates.io rejects a category slug at publish time. Dry runs do not
  check slugs. _Mitigation_: each slug above is in the crates.io category list
  (`visualization`, `graphics`, `wasm`, `rendering`, `gui`, `game-engines`,
  `text-processing`, `development-tools::procedural-macro-helpers`). GUP-598
  repeats the check against `https://crates.io/category_slugs`.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not applicable, nothing renders
      differently
- [ ] `mask wasm-browser`: not needed; the only wasm-size change is an HTML
      comment
- [ ] WASM size: unchanged, no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
