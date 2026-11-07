---
implementer: sonnet
implementer-reason:
  order and tests specified in Decisions; the work is a new const, a default
  switch, and re-blessing goldens by eye
touches:
  [
    crates/gup-core/src/scale/categorical.rs,
    crates/gup-core/src/scale/mod.rs,
    crates/gup-core/src/marks.rs,
    crates/gup-core/src/theme.rs,
    crates/gup-core/src/lib.rs,
    CHANGELOG.md,
  ]
---

# GUP-658: Default Categorical Order Puts the Most Distinct Colours First

## Story Overview

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: 🚧 In
Progress Ready **Created**: 2026-11-07

## Context

`Categorical::okabe_ito()` and every builder default use `OKABE_ITO` in the
published Okabe-Ito order: orange, sky blue, bluish green, blue, vermillion,
reddish purple, yellow, black. In that order series 1 and 5 (orange `#E69F00`,
vermillion `#D55E00`) are near neighbours, and so are series 2 and 4 (sky blue
`#56B4E9`, blue `#0072B2`). The gallery charts `scatter_bubbles` (5 regions) and
`dark_multiline` (5 hosts) therefore show two near-identical pairs. The marks'
default fill (`marks.rs` `DEFAULT_FILL`) is `OKABE_ITO[3]`, the blue, so a
one-series chart is blue while series 1 of a multi-series chart is orange.

`OKABE_ITO_DARK` is built from `OKABE_ITO` by index (black replaced). GUP-625
added `HIGH_CONTRAST` and a test with a CVD simulation (Machado 2009, severity 1
matrices) and CIEDE2000 (`gup-visual-regression/src/color.rs`, `delta_e_2000`);
this story reuses both. About 69 places index `OKABE_ITO[..]` (tests, doctests,
examples).

## Read First

- `crates/gup-core/src/scale/categorical.rs` lines 1–200 (consts, doc comments,
  `Categorical::okabe_ito`, the `themed` field) and its palette tests (about
  lines 440–570).
- `crates/gup-core/src/theme.rs`: the palette test GUP-625 added (grep
  `DEUTERANOPIA`) for the CVD matrices and the ΔE helper to reuse.
- `crates/gup-core/src/marks.rs`: `DEFAULT_FILL`.
- `grep -rn 'OKABE_ITO\[' --include='*.rs' crates examples tests` (the index
  uses to review).

## User Story

> "As a reader of a chart with four or five series, I want the first colours to
> look clearly different from each other, so that I can tell the series apart
> without a legend lookup."

## Acceptance Criteria

### AC1: The distinct order exists and is the default

- [ ] A new `pub const OKABE_ITO_DISTINCT: [Color; 8]` and a dark twin
      `OKABE_ITO_DISTINCT_DARK` in `scale/categorical.rs`, exported where
      `OKABE_ITO_DARK` is (`scale` and the crate root).
- [ ] `OKABE_ITO` and `OKABE_ITO_DARK` are unchanged (values, order, docs say
      "the published order").
- [ ] `Categorical::okabe_ito()`, `Theme::default()` and `Theme::dark()`
      palettes and every builder default use the new consts; `DEFAULT_FILL`
      stays the blue (`#0072B2`), now equal to `OKABE_ITO_DISTINCT[0]`; a
      doctest asserts it.

### AC2: Measured distinctness

- [ ] A test computes CIEDE2000 between every pair of the first four colours of
      `OKABE_ITO_DISTINCT` under normal vision, deuteranopia, protanopia and
      tritanopia (tritanopia matrix from Machado 2009, severity 1, added next to
      the other two), and asserts the minimum is at least the measured floor
      minus 0.5, rounded down (record the measured minimum for each view in the
      test's comment and the story retrospective).
- [ ] A second assertion: among the first five colours, no pair has ΔE00 below
      the smaller of 15 and the measured minimum; the same test asserts that the
      same check on `OKABE_ITO[..5]` fails (shows the old order was worse), with
      its measured minimum recorded.
- [ ] If the measured minimum of the first four under any view is below 10,
      escalate (see triggers).

### AC3: High-contrast palette checked

- [ ] The worker computes the same two measures for `HIGH_CONTRAST[..5]` and
      records them. The palette changes only if a pair among its first five is
      below ΔE00 12 in some view; otherwise it is left alone and the
      retrospective says so.

### AC4: Seen

- [ ] Every golden (including gallery goldens) that changes is re-blessed and
      looked at. List each file in the retrospective with one line on what it
      shows. In `gallery/scatter_bubbles` and `gallery/dark_multiline` the five
      series are visibly distinct, with blue first.
- [ ] No chart with a single series changes colour.

### AC5: Sweep

- [ ] Every `OKABE_ITO[n]` use (tests, doctests, examples, browser scenes under
      `crates/gup-core/wasm-size/`) is reviewed: those that mean "the default
      palette's colour n" move to `OKABE_ITO_DISTINCT[n]`; those that mean the
      published colour keep `OKABE_ITO`.
- [ ] README, tutorial, `examples/README.md` and doc comments that name a series
      colour ("orange line", "sky blue") are corrected to the new order.
- [ ] CHANGELOG `[0.1.0]` gets a "Changed" line (add the heading if absent): the
      default palette order, naming the new consts.

## Decisions

- **Order** (`OKABE_ITO_DISTINCT`): blue `#0072B2`, orange `#E69F00`, bluish
  green `#009E73`, reddish purple `#CC79A7`, sky blue `#56B4E9`, vermillion
  `#D55E00`, yellow `#F0E442`, black `#000000`. The dark twin replaces the black
  with `#E8E8E8`, as `OKABE_ITO_DARK` does. The first four differ in hue and
  lightness; blue first matches the single-series default.
- **Name**: `OKABE_ITO_DISTINCT`. The worker may propose a better name in the
  retrospective but keeps this one unless the orchestrator renames it.
- **Canonical consts stay**: they are public and documented as the published
  order.
- **Ties to the old order** (`OKABE_ITO_DARK` built from `OKABE_ITO` by index)
  are not kept: the dark twin lists its colours.
- **No API beyond two consts**; the `Categorical` constructors keep their
  signatures.

## Escalation Triggers

- Any public API change beyond adding the two consts.
- More than about 40 goldens change: report the count before re-blessing.
- A golden changes in a way other than series colours (layout, text).
- The first four measure below ΔE00 10 in any view.
- A doctest or browser scene depends on the old order in a way that is not an
  index swap.

## Out of Scope

- Reordering `HIGH_CONTRAST` without the AC3 finding.
- New palettes, or sequential and diverging palettes.
- Colour keys past eight (the extension rule in `extend`).

## Technical Tasks

- [ ] Add the consts, switch the defaults, export.
- [ ] Add the tritanopia matrix and the two tests.
- [ ] Measure `HIGH_CONTRAST`; record.
- [ ] Sweep `OKABE_ITO[` uses and colour-naming text.
- [ ] Re-bless goldens one directory at a time; look at each.
- [ ] CHANGELOG.

## Dependencies

### Prerequisite Stories

- GUP-625: High-Contrast Theme ✅ — the CVD matrices and the ΔE test pattern.

## Testing Strategy

- **Unit tests**: AC2's measures, `DEFAULT_FILL` equality.
- **Visual validation**: the re-blessed goldens, each looked at.
- **External API**: a doctest on `OKABE_ITO_DISTINCT` using only `gup::` paths.

## Success Metrics

- [ ] The measured minimum ΔE00 of the first four, per view, is recorded and
      asserted.
- [ ] Both five-series gallery charts show no near-identical pair.

## Risk Assessment

- **Medium**: many goldens move at once. _Mitigation_: re-bless per directory
  and look at one of each kind; escalate over 40.
- **Low**: sky blue fifth is still close to blue first for tritanopes.
  _Mitigation_: the AC2 five-colour floor is measured, not assumed.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (every re-blessed golden)
- [ ] `mask wasm-browser` (browser scenes use the default palette and may check
      pixels)
- [ ] WASM size: `mask wasm-size` scatter gz bytes (two consts added)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Blocked (2026-11-07)

**Question**: AC2's "the old order is worse" assertion cannot hold: measured
CIEDE2000 shows `OKABE_ITO_DISTINCT` is not more distinct than the published
order by minimum pair distance. How should the story proceed?

Measured (Machado 2009 severity 1; minimum pairwise ΔE00; test
`measure_palettes` in `theme.rs`, WIP):

| Palette slice      | normal | deuteranopia | protanopia | tritanopia |
| ------------------ | -----: | -----------: | ---------: | ---------: |
| DISTINCT[..4]      |  38.40 |        16.11 |      12.26 |      11.13 |
| OKABE_ITO[..4]     |  22.31 |        22.27 |      17.91 |      12.07 |
| DISTINCT[..5]      |  22.31 |        15.54 |      12.26 |      11.13 |
| OKABE_ITO[..5]     |  22.24 |        12.52 |      17.91 |      12.07 |
| HIGH_CONTRAST[..4] |  38.40 |        20.53 |      18.20 |      12.07 |
| HIGH_CONTRAST[..5] |  37.02 |        16.11 |      12.26 |      12.07 |

- First four of DISTINCT are above the ΔE 10 trigger in every view (floor
  11.13), so that trigger does not fire.
- Over all views, min of DISTINCT[..5] is 11.13 and of OKABE_ITO[..5] is 12.07.
  AC2 requires the published order's check (threshold min(15, 11.13)) to fail;
  it passes (12.07 >= 11.13). DISTINCT is worse than published under protanopia
  and tritanopia, better under deuteranopia (15.5 vs 12.5) and about equal under
  normal vision (22.3 vs 22.2).
- The premise (orange/vermillion and sky blue/blue near pairs) is only true as a
  visual impression: under normal vision the published first five are already at
  ΔE 22.
- AC3: HIGH_CONTRAST first five are all at or above 12.07 (>= 12), so by the
  story's rule it stays unchanged.

Options:

1. Keep the Decisions order, drop the "old order fails" assertion (replace with
   a recorded comparison), and soften the story's claim. The gallery change is
   cosmetic (blue first, orange second).
2. Search for an order that beats the published one in every view (a quick brute
   force over orderings of the first five, blue first, excluding yellow and
   black early, is cheap) and make that the Decisions order.
3. Abandon the story (the published order is already ΔE >= 12 everywhere).

Recommendation: option 2 if the goal is genuine distinctness (the story's
measure is then meaningful); otherwise option 1 for the blue-first consistency
alone. The code (consts, default switch, exports, marks, theme) is in the WIP
commit; goldens are not yet re-blessed and the sweep is not started.
