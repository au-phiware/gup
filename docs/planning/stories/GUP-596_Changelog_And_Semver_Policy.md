---
implementer: sonnet
implementer-reason:
  docs only; the file format, sections and policy rules are fixed in Decisions,
  and the content comes from named sources
touches: []
---

# GUP-596: CHANGELOG and a 0.x Semver Policy

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 📋 Planned
**Created**: 2026-11-02

## Context

There is no `CHANGELOG.md` and no stated versioning policy. Release 0.1 is the
first published version. A user needs two things from the start: what 0.1
contains, and what a later 0.x number promises. The seven crates move in
lockstep with exact internal requirements (`=0.1.0`, GUP-588), and `gup`
re-exports `wgpu` as `gup::wgpu`. Both facts affect what counts as a breaking
change, so the policy has to say so.

## Read First

- `docs/planning/stories/INDEX.md`: tracks N0–N3 (the one-liners are the raw
  material for "Added").
- `README.md` "Key Features" and "Not yet".
- `crates/gup-core/PERF_BUDGETS.md` "Budget table" (only for figures the
  CHANGELOG may quote; quote none it does not list).
- GUP-588's Decisions (lockstep versions, `rust-version`).

## User Story

> "As a user upgrading Gup, I want a changelog and a written versioning policy,
> so that I know what changed and whether an upgrade can break my code."

## Acceptance Criteria

### AC1: CHANGELOG.md

- [ ] A root `CHANGELOG.md` in the format of Decisions, with a
      `## [0.1.0] - Unreleased` section and the policy section.
- [ ] Its "Added" list covers every builder (`scatter`, `line`, `bar`, `barh`,
      `area`, `histogram`, `heatmap`, `boxplot`), `Plot` and `Selection`, the
      four marks plus `Area`, text layers, `#[wgsl_function]` and
      `#[derive(Mark)]`, the scale family, legends, themes (`Theme::dark`), the
      targets (window, PNG, SVG, texture, canvas), interaction (picking,
      tooltips, zoom, pan, brush), streaming (`append`, `Window`), `gup-egui`,
      `gup-bevy`, `Plot::describe` with SVG and canvas text alternatives
      (GUP-592/593), and the tutorials and gallery. Each item is one line, in
      user terms, with no story numbers and no "RFC".
- [ ] "Known limitations" repeats the README's "Not yet" items as they stand
      when this story runs.
- [ ] `grep -n 'GUP-\|RFC' CHANGELOG.md` prints nothing. `prettier --check` and
      `mdl` pass.

### AC2: Linked

- [ ] `docs/README.md`'s reading guide lists `CHANGELOG.md` in one line. The
      root `README.md` is not edited here (GUP-597 links it).

## Decisions

- **Format**: [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/).
  Sections in this order: a one-paragraph intro, `## Versioning`,
  `## [Unreleased]` (empty), `## [0.1.0] - Unreleased` with `### Added` and
  `### Known limitations`. GUP-598 replaces `Unreleased` in the 0.1.0 heading
  with the date when the owner publishes.
- **The policy**, as the `## Versioning` section's bullets:
  - All seven crates share one version and are released together. Each depends
    on the others at exactly that version.
  - Before 1.0, a release that breaks the public API raises the minor version
    (0.1 → 0.2). A release that only adds or fixes raises the patch version
    (0.1.0 → 0.1.1).
  - The public API is every public item reachable from `gup`, including its
    re-exports of `gup-core`, except items marked `#[doc(hidden)]` and the
    `__private` module. Changes to `gup-core-macros`, `gup-wgsl` and `gup-text`
    that are invisible through `gup` follow the same numbers, because of the
    lockstep.
  - `gup::wgpu` is a re-export, so a wgpu major upgrade is a breaking change and
    raises the minor version.
  - Raising `rust-version` (the minimum Rust version, 1.93 for 0.1) raises the
    minor version.
  - Rendered output is not API. Pixel-level changes (antialiasing, layout
    tweaks) can come in a patch, and are listed under "Changed".
  - Default feature changes and removals are breaking.
- **No automation** here. A semver check against the published 0.1 baseline
  (`cargo-semver-checks`) is a follow-up after the first release.

## Escalation Triggers

- An "Added" item cannot be stated without claiming something `PERF_BUDGETS.md`
  or a test does not show (for example, a scale or speed claim). Leave it out
  and list it in the Implementation Summary.

## Out of Scope

- The README: GUP-597.
- Release dates, tags and the GitHub release: owner steps after GUP-598.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30
      minutes.
- [ ] Write `CHANGELOG.md` (AC1). Commit.
- [ ] `docs/README.md` line (AC2); prettier and mdl.

## Dependencies

### Prerequisite Stories

- None blocking. Runs best after GUP-592 and GUP-593 are merged, so that the
  accessibility items are true. If they are not merged, the item reads as a
  limitation instead.

### Enables Stories

- GUP-597: README for release 0.1 — links the CHANGELOG.
- GUP-598: Release checklist — checks the CHANGELOG's version.

## Testing Strategy

- **Docs checks**: `prettier --check`, `mdl`, the grep in AC1.

## Success Metrics

- [ ] A user can read what 0.1 contains, and the rules for every later number,
      on one page.

## Risk Assessment

- **Low**: the list drifts from the code before the release. _Mitigation_:
  GUP-598's checklist re-reads it against the README.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: not needed, docs only (`prettier` and `mdl` pass)
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: not affected
- [ ] Rendered output verified by eye: not applicable
- [ ] `mask wasm-browser`: not needed, docs only
- [ ] WASM size: unchanged, no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
