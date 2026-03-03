# GUP-272: WCAG 2.1 AA Compliance Validation

## Story Overview

**Initiative**: Accessibility **Status**: 📋 Planned **Created**: 2025-07-23

## Context

Gup's README states "WCAG 2.1 AA compliance from day one" as a core commitment.
A substantial body of accessibility work has been completed across the
Accessibility initiative — GUP-016 established the core accessibility system and
`FocusManager`; GUP-111 implemented automatic ARIA tree generation; GUP-112 and
GUP-114–117 integrated platform-native accessibility APIs on macOS, Windows,
Linux, and the web; GUP-122 defined and executed a manual screen reader testing
protocol; GUP-124 delivered perceptually accurate colour descriptions; and
GUP-127 added focusable elements for individual data points enabling full
keyboard navigation.

Despite this extensive work, the overall WCAG 2.1 AA success criteria have never
been evaluated systematically as a complete set. Individual stories addressed
specific criteria opportunistically, but no audit has been conducted to verify
every applicable principle — Perceivable, Operable, Understandable, Robust —
against Gup's actual implementation. Without a formal audit there is no evidence
base for the compliance claim, no record of which criteria are not applicable to
a GPU-rendered visualization library, and no mechanism to prevent regressions.

This story conducts that audit end-to-end: maps every WCAG 2.1 AA success
criterion to Gup's feature set, produces a documented Pass / Fail / N-A verdict
with evidence for each, fixes any simple gaps found during the audit, opens
follow-up stories for complex gaps that require dedicated work, integrates
automated accessibility checks into CI, and publishes a WCAG 2.1 AA Conformance
Statement and a consumer-facing accessibility testing guide.

## User Story

> "As a visualization developer, I want a published WCAG 2.1 AA Conformance
> Statement backed by a systematic audit so that I can confidently rely on Gup
> in products that must meet accessibility regulations."

> "As an end user of a Gup-powered application, I want the data visualizations I
> encounter to meet WCAG 2.1 AA so that I can perceive, navigate, and understand
> the data using assistive technologies."

> "As a Gup maintainer, I want automated accessibility checks in CI so that
> newly introduced features cannot silently regress compliance."

## Acceptance Criteria

### AC1: Complete WCAG 2.1 AA Criterion Mapping

- [ ] All 50 WCAG 2.1 AA success criteria (levels A and AA) are listed in a
      structured audit document (`docs/accessibility/WCAG_2_1_AA_AUDIT.md`)
- [ ] Each criterion is assigned exactly one of: **Pass**, **Fail**, or **N/A**
- [ ] Every **Pass** verdict is accompanied by a pointer to the implementing
      story or code location that satisfies it
- [ ] Every **Fail** verdict is accompanied by a description of the gap and
      either a committed fix (within this story) or a reference to a filed
      follow-up story
- [ ] Every **N/A** verdict is accompanied by a justification explaining why the
      criterion does not apply to a GPU-rendered visualization library (e.g.,
      criteria concerning video captions where Gup produces no video)
- [ ] The audit document records the WCAG specification version (2.1) and the
      date the audit was completed

### AC2: Gap Remediation (Simple Fixes)

- [ ] All **Fail** verdicts whose fix is scoped within this story's complexity
      budget are resolved before the Conformance Statement is finalised
- [ ] Each fix is accompanied by a corresponding regression test or automated
      check that would catch reintroduction of the gap
- [ ] Complex gaps that cannot be addressed within this story each have a
      follow-up story filed and referenced in the audit document; no complex gap
      is silently left untracked

### AC3: WCAG 2.1 AA Conformance Statement

- [ ] A conformance statement document is published at
      `docs/accessibility/WCAG_2_1_AA_CONFORMANCE.md`
- [ ] The statement follows the WCAG 2.1 specification's recommended format
      (conformance level, product name, date, technologies relied upon, known
      limitations, third-party content note)
- [ ] Conformance level is declared truthfully: "Supports" / "Supports with
      Exceptions" / "Does Not Support" per VPAT/ACR convention, with "Supports
      with Exceptions" permitted only when all exceptions are documented in the
      audit
- [ ] The statement identifies which WCAG 2.1 AA criteria are not applicable to
      Gup and why

### AC4: Automated Accessibility Checks in CI

- [ ] At least one automated accessibility-checking tool is integrated into the
      CI pipeline and runs on every pull request
- [ ] For the web target: an axe-core (or equivalent) scan of the WASM
      integration's DOM overlay (from GUP-117) is executed and must pass with
      zero violations at the "critical" and "serious" severity levels
- [ ] For the native target: any available OS accessibility API validation
      (e.g., macOS Accessibility Inspector scripted checks, AT-SPI2
      `at-spi2-atk` verification) is run and must produce zero errors
- [ ] CI failure on a new accessibility violation blocks merge
- [ ] The CI check is documented in `docs/accessibility/CI_ACCESSIBILITY.md`,
      including how to run it locally and how to interpret failures

### AC5: Consumer-Facing Accessibility Testing Guide

- [ ] A guide is published at `docs/accessibility/TESTING_GUIDE.md`
- [ ] The guide explains how downstream consumers can validate WCAG compliance
      in their own applications built on Gup
- [ ] The guide covers: running axe-core against web builds, using screen
      readers with Gup-powered apps (NVDA/JAWS/VoiceOver, building on GUP-122),
      keyboard-only navigation testing, and colour contrast checking
- [ ] The guide includes at least one worked example using an existing Gup
      example program

## Technical Tasks

- [ ] Create `docs/accessibility/` directory if it does not already exist
- [ ] Enumerate all 50 WCAG 2.1 AA success criteria (download the official list
      from the WCAG 2.1 specification or use the W3C quick reference)
- [ ] For each criterion, cross-reference existing Accessibility initiative
      stories and source code to determine Pass / Fail / N/A
- [ ] Produce the structured audit document (`WCAG_2_1_AA_AUDIT.md`) with
      verdict + evidence for every criterion
- [ ] Triage Fail verdicts: identify which can be fixed within this story (e.g.,
      missing `lang` attribute on DOM overlay, incorrect ARIA role assignments,
      contrast ratio shortfalls in default theme) vs. which require dedicated
      stories
- [ ] Implement simple fixes identified during triage; write regression tests
      for each
- [ ] File follow-up stories for any complex gaps; record story IDs in the audit
      document
- [ ] Draft `WCAG_2_1_AA_CONFORMANCE.md` conformance statement once all
      remediable failures are fixed
- [ ] Evaluate CI tooling options: axe-core CLI/headless browser for web,
      AT-SPI2 tooling for Linux, Accessibility Inspector for macOS
- [ ] Integrate chosen CI tool(s) into the CI pipeline (GitHub Actions or
      equivalent); configure severity thresholds and failure conditions
- [ ] Write `CI_ACCESSIBILITY.md` documenting the CI check
- [ ] Write `TESTING_GUIDE.md` consumer testing guide including a worked example
- [ ] Update `docs/README.md` to link to the new accessibility documents

## Dependencies

### Prerequisite Stories

- GUP-016: Core Accessibility System ✅ — provides `AccessibilitySystem`,
  `FocusManager`, and `ContrastMode` that are the foundation audited here
- GUP-111: Automatic ARIA Generation ✅ — provides the ARIA tree whose
  correctness is a primary subject of the audit
- GUP-112: Platform Accessibility Integration ✅ — provides OS-level integration
  points checked against WCAG Robust criteria
- GUP-122: Manual Screen Reader Testing Execution ✅ — provides the screen
  reader compatibility matrix used as evidence in the audit
- GUP-124: Enhanced Color Description ✅ — provides accurate colour naming that
  informs the 1.4.1 (Use of Color) and 1.4.3 (Contrast) verdicts
- GUP-127: Focus Elements for Data Points ✅ — provides keyboard navigation of
  data points, directly satisfying WCAG 2.1.1 (Keyboard)

### Enables Stories

- Any follow-up stories filed during gap remediation (GUP numbers assigned at
  audit time)

## Testing Strategy

- **Automated (CI)**: axe-core headless scan of the web/WASM DOM overlay;
  AT-SPI2 validation on Linux CI runners. Both must pass with zero critical or
  serious violations.
- **Unit tests**: Regression tests for each simple gap fixed within this story
  (e.g., correct ARIA roles, colour contrast ratios in default theme).
- **Manual validation**: Use the protocol from GUP-122 to spot-check a
  representative set of WCAG criteria with NVDA and VoiceOver before finalising
  the conformance statement.
- **Document review**: The audit document and conformance statement are reviewed
  against the WCAG 2.1 specification by at least one contributor not involved in
  the original implementation, to catch misclassifications.

## Success Metrics

- [ ] All 50 WCAG 2.1 AA success criteria are assigned a verdict in the audit
      document; no criterion is left blank or unresolved
- [ ] Zero open **Fail** verdicts without either a committed fix or a filed
      follow-up story at the time the conformance statement is published
- [ ] Automated CI accessibility checks run on every PR and produce zero
      blocking violations on the existing codebase at the time this story closes
- [ ] `WCAG_2_1_AA_CONFORMANCE.md` is published and declares at minimum
      "Supports with Exceptions" for WCAG 2.1 AA

## Risk Assessment

- **Medium**: Some WCAG criteria are ambiguous when applied to a GPU-rendered
  canvas rather than a traditional DOM UI (e.g., 1.3.4 Orientation, 1.4.10
  Reflow). Determinations of N/A require careful justification to avoid
  improperly discounting applicable criteria. _Mitigation_: Follow W3C's
  "WCAG2ICT" guidance on applying WCAG to non-web software; document the
  rationale for each N/A in full.

- **Medium**: Automated tooling (axe-core) can only scan the DOM overlay
  produced by GUP-117; it cannot inspect the GPU framebuffer directly. GPU-side
  rendering issues (e.g., colour contrast within the canvas) require manual
  checks or custom tooling. _Mitigation_: Supplement automated scans with manual
  contrast ratio measurements using the existing colour description system
  (GUP-124) and theme contrast values from GUP-016.

- **Low**: The CI integration may require a headless browser environment
  (Chromium/Firefox) that is not currently available in the CI runners. Spinning
  up a headless browser adds pipeline complexity and execution time.
  _Mitigation_: Use a lightweight axe-core CLI container or Playwright's
  headless mode, which are well-supported in most CI environments.

- **Low**: The audit may uncover gaps significant enough that the project cannot
  honestly claim "Supports" for WCAG 2.1 AA in full. If so, a "Supports with
  Exceptions" declaration with a clear remediation roadmap is still a meaningful
  and honest compliance posture. _Mitigation_: Scope this story to deliver an
  honest, evidence-based assessment rather than a rubber-stamp. Follow-up
  stories provide the remediation path.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] `docs/accessibility/WCAG_2_1_AA_AUDIT.md` committed and complete
- [ ] `docs/accessibility/WCAG_2_1_AA_CONFORMANCE.md` committed and published
- [ ] `docs/accessibility/CI_ACCESSIBILITY.md` committed and CI check is live
- [ ] `docs/accessibility/TESTING_GUIDE.md` committed with worked example
- [ ] Any follow-up stories for complex gaps are filed and their IDs recorded in
      the audit document
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
