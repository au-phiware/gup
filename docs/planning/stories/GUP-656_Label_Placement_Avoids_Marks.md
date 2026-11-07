---
implementer: sonnet
implementer-reason:
  design settled in Decisions (a constant-radius setter seeding `Kept`); one
  function plus a setter, patterns from GUP-527 and GUP-611
touches:
  [
    crates/gup-core/src/text_layer.rs,
    crates/gup-core/tests/text_halo.rs,
    CHANGELOG.md,
  ]
---

# GUP-656: Label Placement Avoids Covering Other Marks

## Story Overview

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: 🚧 In
Progress **Created**: 2026-11-07

## Context

GUP-527's `hide_overlaps(true)` and GUP-611's `fallbacks` keep labels from
overlapping each other. Nothing keeps a label off the marks. The
`gup_core/text_fallbacks` golden (`tests/golden/gup_core/text_fallbacks.png`)
shows labels such as "61.7", "30.4", "0.2", "37.5" and "9.0" drawn on top of
neighbouring discs, and a label beside its point can sit nearer another point
than its own, so the reader cannot tell which point it names. On the 1,000-point
scatter GUP-611 raised the kept labels from 521 to 599.

A `TextLayer<T>` has no link to the mark layer it annotates. It holds its own
`rows`, its own `x`/`y` closures and no radius (`TextLayer` in `text_layer.rs`);
`Plot::text` pushes it into `annotations`, separate from the data layers, whose
circle radius is a `Selection` attribute (`.radius(Px(4.0))`) that may be a
per-row channel. So mark sizes are not available at layout time without new
plumbing between layers (a design question). This story takes the cheap route:
the layer is told a constant px radius and treats each of its own rows' pixel
positions as a disc of that radius. That fits the common case (a labelled
scatter whose labels and discs share rows) and needs no cross-layer plumbing.

## Read First

- `crates/gup-core/src/text_layer.rs`: `TextLayer` and its fields (lines
  146–170), `candidates` (200–207), `fallbacks` and `hide_overlaps` docs
  (440–530), `Kept` (680–735), `runs` (800–895: the `kept` creation, the
  `ev.text` loop, `kept.overlaps`/`kept.push`), `reach`/`edges` (896–960).
- `crates/gup-core/tests/text_halo.rs`: `placed_scatter`, `FALLBACKS`,
  `overlap`, `fallbacks_keep_more_labels_and_none_overlap` (342–415) and
  `text_fallbacks_golden` (490–531).
- GUP-611 story, "Implementation Summary" and "Retrospective" (cost came from
  the edge pass; time pieces before tuning).

## User Story

> "As a visualization developer labelling a scatter, I want labels kept off the
> points, so that each label is readable and clearly names its own point."

## Acceptance Criteria

### AC1: The setter and the rule

- [ ] `TextLayer::avoid_points(radius: Px)` exists, documented with a doctest
      that builds a plot with circles and a text layer, resolves, and asserts no
      annotation box overlaps a disc box. Exercised from an external crate (the
      doctest, or a test under `crates/gup-core/tests/`).
- [ ] With `hide_overlaps(true)`, `runs` seeds `Kept` with one square box of
      side `2 * radius` centred on the pixel position of every row whose
      position is finite (rows with empty text included), before any label is
      placed. A candidate whose box (inflated by the halo) overlaps a seeded
      disc, or a kept label, is rejected, exactly as for kept labels, and the
      next fallback is tried. A label hidden by this rule is hidden, not moved
      without a fallback.
- [ ] Without `avoid_points`, and without `hide_overlaps(true)`, the output
      equals today's byte for byte (the existing `text_halo` goldens and tests
      pass unchanged, 0 pixels differ).

### AC2: Measured on the test scatter

- [ ] On the 1,000-point scatter of `placed_scatter` (800×600, `FALLBACKS`,
      `.avoid_points(Px(4.0))`, circles of `.radius(Px(4.0))`), no kept label
      box overlaps the bounding box of any row's disc. The test checks every
      pair and prints the kept count against GUP-611's 599.
- [ ] Nearest-point rule: for every kept label, the distance from its box to its
      own point is no greater than its distance to any other row's point, or, if
      that cannot hold for all labels (see Escalation Triggers), the fraction of
      kept labels that fail it is printed with and without `avoid_points`, and
      with it is under 5% of kept labels.
- [ ] Determinism: two resolves give identical runs (row order, then candidate
      order).

### AC3: Cost

- [ ] Release, same session, 3 runs, 1,000 labels with 4 candidates and 1,000
      seeded discs: the cost is under +0.3 ms over GUP-611's fallbacks run (the
      metric in `crates/gup-core/tests/text_layer.rs`, which prints and does not
      assert).

### AC4: Seen

- [ ] The `gup_core/text_fallbacks` golden is re-recorded with `avoid_points`
      (or a new `gup_core/text_avoid_points` golden is added next to it, keeping
      the old one). The implementer reads the PNG and reports: no label touches
      a disc, every kept label sits beside its own disc, and how many labels of
      the 300 are kept against the old golden.

### AC5: Changelog and defaults sweep

- [ ] `CHANGELOG.md` `[0.1.0]` "Added" has one line for
      `TextLayer::avoid_points`.
- [ ] Search tests, examples, gallery, browser scenes and doctests for text
      layers with `hide_overlaps(true)` that annotate circles (grep
      `hide_overlaps`); none is changed (the setter is opt-in), and the story
      says so. Gallery or tutorial charts that would clearly gain are listed in
      the retrospective, not edited.

## Decisions

- **Opt-in setter, not part of `hide_overlaps`.** `avoid_points(Px)` is separate
  because the text layer cannot know mark sizes (Context), the radius must be
  passed, and a default change would alter every existing culled output (the
  GUP-611 compatibility rule: no new setter, same bytes). A `bool` is not enough
  for the same reason.
- **Signature**: `pub fn avoid_points(&mut self, radius: Px) -> &mut Self`; a
  `radius` of 0 or less (or NaN) means off (default 0). It bumps `revision` via
  `changed()` like every setter. A new field `avoid: f32`.
- **Requires `hide_overlaps(true)`**, ignored otherwise, like `fallbacks`
  (documented). Edge room (`reach`/`edges`) is unchanged: seeded discs do not
  move reach.
- **The discs are the layer's own rows**, at the layer's `x`/`y` pixel
  positions, not the offset label positions. All rows seed, whether or not their
  label is drawn: a point with no label is still an obstacle. Rows with
  non-finite positions are skipped.
- **Seeding area**: the existing `Kept::new(plot.inflate(halo), 4 * size)`;
  discs partly outside the grid are clamped by `span`. A disc box is not
  clipped.
- **Own disc is an obstacle too**, with no exemption: the user picks `dy`/`dx`
  large enough to clear their own mark (the doctest and test use offsets above
  the radius). Reason: simple, deterministic, and a label on its own point is
  the worst case for legibility.
- **Constant radius only.** A per-row radius (a closure or reading the mark
  layer's size channel) is deliberately left out (Out of Scope).
- Seeded discs are pushed with `Kept::push`; no second grid, no change to
  `Kept`'s API.

## Escalation Triggers

- The nearest-point rule cannot be met for all labels, and the fraction with
  `avoid_points` is not under 5%: stop and report the numbers (a rule that needs
  a nearest-neighbour query is a design question).
- Seeding the grid costs more than +0.3 ms and the cost is not in `Kept::push`
  (grid cells too coarse for dense discs, e.g. 64×64 cap with 1,000 discs):
  report, do not change `Kept`'s cell sizing without asking.
- Kept labels fall by more than a third against GUP-611's 599 (discs plus labels
  leave too little room): report; the radius or offsets may be wrong, not the
  rule.
- Anything needs the mark layer's data (radius, size channel) at layout time.

## Out of Scope

- Per-row or data-driven radii, and finding the target mark layer's size
  automatically (a design story: a link between `TextLayer` and a data layer;
  none exists today).
- Avoiding marks other than circles (bars, lines, areas), and the axes or
  legends.
- More fallback candidates (diagonals), GUP-611's retrospective note.
- Changing `hide_overlaps` or `fallbacks` defaults.

## Technical Tasks

- [ ] Add field `avoid: f32` (default 0.0), the setter with a doc comment and
      doctest, and `changed()`.
- [ ] In `runs`, after creating `kept` and before the row loop, seed discs when
      `avoid > 0`: for each row `i`, pixel position via `pixel(&ev.x, i, x)`,
      `pixel(&ev.y, i, y)`; skip non-finite; `kept.push(Rect::from_edges(..))`.
- [ ] Tests in `tests/text_halo.rs`: AC2 checks on `placed_scatter` extended
      with an avoid argument or a sibling builder; the golden.
- [ ] Cost run per AC3; CHANGELOG line; `mask all-fix`.

## Dependencies

### Prerequisite Stories

- GUP-611 ✅ — `fallbacks`, the inline-box `Kept` grid and the cost metric.
- GUP-527 ✅ — `hide_overlaps` and `Kept`.

### Enables Stories

- None known; a follow-up could link the text layer to a mark layer's radii.

## Testing Strategy

- **Unit/integration**: `Layout::texts` of the 1,000-point scatter for the
  overlap and nearest-point rules; determinism; default-unchanged check against
  the existing tests.
- **Visual**: the golden read by eye (AC4).
- **Performance**: the cost metric in `tests/text_layer.rs`, release, same
  session A/B.

## Success Metrics

- [ ] No kept label touches a disc in the test scatter, and the readability rule
      of AC2 holds as stated.
- [ ] Defaults byte-identical; cost under +0.3 ms.

## Risk Assessment

- **Medium**: a dense scatter has little room, so many labels hide and the gain
  depends on the radius and offsets. _Mitigation_: the escalation triggers
  report the kept count; the setter is opt-in.
- **Low**: a constant radius misfits charts whose marks vary in size.
  _Mitigation_: documented, and a follow-up is named in Out of Scope.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the golden PNG read)
- [ ] `mask wasm-browser`: not needed; no layout, input or wasm change beyond a
      default-off setter
- [ ] WASM size: `mask wasm-size` scatter gz bytes, or "unchanged: no library
      code change"
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Blocked (2026-11-08)

**Question**: two Escalation Triggers fire on the 1,000-point scatter (800x600,
discs and boxes of radius 4, offsets dy=-5 and fallbacks 6 px, so a label can
clear its own disc). Do we relax the AC2 nearest-point rule and the kept-label
expectation, or add a nearest-neighbour query?

**Numbers** (`avoid_points_keeps_labels_off_the_discs`, `--nocapture`):

- Kept labels: 374 with `avoid_points`, 647 without (same offsets), GUP-611's
  599 on its offsets. 374 is 38% below 599 (42% below 647): more than a third.
- Labels touching a disc: 563 without, 0 with (the seeding rule works).
- Nearest-point rule: a box nearer another row's point than its own: 597/647
  (92%) without, 158/374 (42%) with. Far above 5%.
- The 300-point golden scene with `.avoid_points(Px(4.0))` keeps 156 labels; the
  render (not blessed) shows no label on a disc and labels beside their points.

**Options**: (a) accept the dense-scatter numbers: drop the 5% rule to a printed
figure and the kept-count floor to the measured one; the golden is the visible
win. (b) A real nearest-point rule (a nearest-neighbour query, reject a
candidate nearer another point than its own): a design question. (c) Smaller
discs/offsets: the 1,000 points have ~18% disc coverage, so little room is left.
**Recommendation**: (a) for the count, and (b) as a follow-up (GUP-662).

State: WIP commit; seeding and `avoid_points` are implemented with a doctest;
the nearest-point assertion is commented out and the kept-count assertion is
disabled; no golden blessed, CHANGELOG not yet edited.
