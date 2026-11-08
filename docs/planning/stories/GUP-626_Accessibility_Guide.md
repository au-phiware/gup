---
implementer: sonnet
implementer-reason:
  a tested guide over APIs that exist by then, following the tutorials'
  doctest-include pattern; outline settled in Decisions
touches:
  [
    docs/ACCESSIBILITY.md,
    src/lib.rs,
    README.md,
    CHANGELOG.md,
    docs/archive/README.md,
  ]
---

# GUP-626: Accessibility Guide

## Story Overview

**Initiative**: Track N4: Accessibility **Status**: ✅ Complete (2026-11-08)
**Created**: 2026-11-04

## Context

When GUP-619 to GUP-625 land, Gup has a text alternative, keyboard focus over
rows, keyboard zoom and pan, row announcements, a browser live region and data
table, and a high-contrast theme. None of it is documented in one place. The
only accessibility documents are the archived guides in
`docs/archive/accessibility/`, which describe the system deleted in GUP-488
(AT-SPI, UI Automation, sonification) and a WCAG conformance claim for it. The
README's "Not yet" list (line 55) and the CHANGELOG's limitations still say
there is no keyboard navigation, no data table and no high-contrast theme.

## Read First

- `README.md` lines 45–60 ("Not yet").
- `CHANGELOG.md` lines 20–80 (`[Unreleased]` and 0.1.0's limitations).
- `src/lib.rs` lines 330–380 (the `cfg(doctest)` modules that include the
  tutorials; the pattern for including a guide).
- The story files of GUP-592, GUP-593 and GUP-619 to GUP-625, "Decisions" only.

## User Story

> "As a developer who must ship accessible charts, I want one guide that says
> what Gup does for accessibility, what I must do, and what it does not do, so
> that I can meet my obligations without reading the source."

## Acceptance Criteria

### AC1: The guide

- [x] `docs/ACCESSIBILITY.md` with the sections of Decisions, each feature shown
      in a short compiled example.
- [x] Every Rust block in it is a doctest of `gup`, included from `src/lib.rs`
      in a `#[cfg(doctest)]` module as the tutorials are, and
      `cargo test --doc -p gup` passes.
- [x] Examples that need a browser are `no_run` and compile for the host target
      only where they compile today (mark wasm-only code `ignore` with a
      one-line reason, at most one block).

### AC2: Other docs agree

- [x] README "Not yet" no longer lists keyboard navigation, a data table or a
      high-contrast theme; it links the guide from its feature list.
- [x] `CHANGELOG.md` `[Unreleased]` has an "Added" entry per feature (keyboard
      focus, keyboard view keys, row announcements, hosts forwarding keys, the
      live region, the data table, the high-contrast theme).
- [x] `docs/archive/README.md` says the archived accessibility guides describe
      deleted code and points to `docs/ACCESSIBILITY.md`.
- [x] `git grep -n "no keyboard navigation\|keyboard navigation, a data table" -- README.md CHANGELOG.md docs/*.md`
      shows only the historical 0.1.0 limitation in the CHANGELOG.

## Decisions

- **Sections**, in order: "What Gup does" (a table: feature, where it works:
  PNG, SVG, window, egui, browser); "Text alternatives" (`describe`,
  `.description(..)`, SVG `<title>`/`<desc>`, canvas `aria-label`); "Keyboard"
  (the keys, focus order, the focus ring, `focusable(false)`); "Screen readers"
  (`focus_text`, the live region, the data table and its 1,000-row cap); "Colour
  and contrast" (`Theme::high_contrast`, the palette's contrast and CVD figures,
  Okabe-Ito by default); "What you must do" (give charts titles and axis titles,
  set `.description(..)` when the generated one is not enough, do not rely on
  colour alone, keep tooltips meaningful as they are announced); "Not supported"
  (Bevy input, platform accessibility APIs such as AT-SPI and UI Automation for
  native windows, patterns as a second channel, sonification).
- **No conformance claim**: the guide states what each feature does, not "WCAG
  AA compliant"; conformance depends on the page or app around the chart.
- **Location**: `docs/ACCESSIBILITY.md`, beside the integration guides.

## Escalation Triggers

- A feature named in Decisions did not land as its story described (the guide
  would document something that does not exist).

## Out of Scope

- New accessibility features.
- Deleting the archived guides (git keeps them; the archive README points away).

## Technical Tasks

- [x] Write the guide and its examples; include it in `src/lib.rs`.
- [x] README, CHANGELOG and archive README edits.

## Dependencies

### Prerequisite Stories

- GUP-619 ✅, GUP-620 ✅, GUP-621 ✅, GUP-622 ✅, GUP-623 ✅, GUP-624 ✅,
  GUP-625 ✅ — the features documented.

## Testing Strategy

- **Doctests**: every block in the guide.

## Success Metrics

- [x] One page answers "what does Gup do for accessibility?" with working code.

## Risk Assessment

- **Low**: documentation over shipped APIs. _Mitigation_: doctests fail if an
  API differs from the guide.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test --doc -p gup` and
      `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, documentation only
- [x] `mask wasm-browser`: not needed, documentation only
- [x] WASM size: unchanged: no library code change
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `docs/ACCESSIBILITY.md`: the Decisions' sections in order, plus a closing
  "Tested and untested" section (no screen reader has been run by hand). Seven
  Rust blocks are doctests (included by `src/lib.rs`, module
  `accessibility_docs`); the one wasm-only block is `ignore`.
- README (feature bullet links the guide; "Not yet" item removed), CHANGELOG
  (`[Unreleased]` "Added" entries; the 0.1.0 known limitation rewritten to the
  real ones), `docs/archive/README.md`, `docs/README.md` and `docs/INDEX.md`.
- Tests: `cargo test -p gup --doc accessibility` 6 passed, 1 ignored.

## Retrospective

**Completed**: 2026-11-08

### Key Technical Learnings

#### Guides as doctests catch wording drift

- **Challenge**: the guide quotes strings (caption, focus text) that are easy to
  get slightly wrong.
- **Solution**: every block asserts the exact string. Two first-draft errors
  surfaced at once: `Plot::line`, not `lines`, and the caption groups digits
  ("first 1,000 of 2,500 rows"), not "1000 of 2500".
- **Pattern**: write the prose, then paste the asserted values back into it.

### Architectural Decisions

#### The guide states absences from the code, not from tests

- **Decision**: "no OS preference following", "Bevy does not forward keys" and
  "nothing speaks `focus_text` outside the browser" are stated plainly.
- **Reasoning**: the story's Decisions and the N4 summaries say so, and no code
  reads such a preference.
- **Trade-off**: no test proves an absence; a later feature makes the sentence
  false without failing anything.

### Development Workflow Insights

- `mdl` rejects `:-:` table alignment (MD057); use plain `---` separators.
- Not verified: the `ignore`d wasm-only block (`gup::web::show`) was not
  compiled; the `mask wasm-browser` smoke covers the real calls. No screen
  reader was run.

### Follow-up Stories

None. The guide's limits point at GUP-667 (the CPU pick bound) and the hosts'
platform-API routing is out of scope.
