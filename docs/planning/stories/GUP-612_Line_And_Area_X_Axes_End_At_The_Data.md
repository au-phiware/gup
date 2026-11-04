---
implementer: sonnet
implementer-reason:
  a third per-axis fit policy beside GUP-561's round_to_ticks, set by two
  builders; the rule and its exceptions are settled in Decisions
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/builder/line.rs,
    crates/gup-core/src/builder/area.rs,
    examples/gallery/area_stacked.rs,
    tests/golden/,
  ]
---

# GUP-612: Line and Area X Axes End at the Data

## Story Overview

**Initiative**: Polish after 0.1 **Status**: 🚧 In Progress **Created**:
2026-11-04

## Context

Every automatic position scale is rounded outward to its 10-tick step after the
domain is fitted (`Plot::resolve`, "1–2. Domains from column stats, then nice",
`crates/gup-core/src/plot.rs` around line 2055: `s.nice()` unless the axis has
GUP-561's `tick_ends` flag, which calls `nice_to`). For a scatter that is right:
the outermost points sit inside the frame. For the x axis of a line or area
chart it is not: the series starts and ends short of the plot edges, leaving an
empty strip that reads as missing data. The gallery's `area_stacked` example
(data 2005–2024) had to pin `.x_scale(Linear::new().domain(2005.0, 2024.0))`,
because the automatic axis ran 2004–2024 and the stack began a year in.

Bar and histogram value axes round out to a labelled tick on purpose (GUP-561);
that stays. This story adds a third per-axis policy, "extent", and sets it on
the x axis of the line and area builders.

## Read First

- `crates/gup-core/src/plot.rs`: the `tick_ends` field (around line 212), its
  setter `round_to_ticks` (around line 533), the nice loop in `resolve` (around
  line 2055) and `round_out` (around line 2373).
- `crates/gup-core/src/scale/linear.rs`: `nice` (around line 191), to see the
  zero-width padding this story must keep.
- `crates/gup-core/src/builder/line.rs` (`pub fn line`, line 59) and
  `crates/gup-core/src/builder/area.rs` (`pub fn area`, line 125).
- `crates/gup-core/src/builder/bar.rs` lines 325–345: how a builder sets
  `round_to_ticks` in its constructor (the pattern to follow).

## User Story

> "As a visualization developer, I want a line or area chart's x axis to start
> and end where my series does, so that the chart has no empty strip at either
> end and I do not have to pin the domain by hand."

## Acceptance Criteria

### AC1: Extent policy

- [ ] `Plot` stores one `AxisFit` per axis (x, y, right y) in place of
      `tick_ends: [bool; 3]` (see Decisions); `round_to_ticks(axis)` sets
      `AxisFit::Ticks` and a new crate-private `fit_extent(axis)` sets
      `AxisFit::Extent`.
- [ ] An automatic axis with `AxisFit::Extent` keeps the fitted extent: no
      `nice`, no `nice_to`, no `round_out`. A zero-width extent (one distinct
      value) falls back to `nice()`, so a single-row line still has a domain.
- [ ] Widening for mark overhang (GUP-528/529) still applies to an extent axis,
      so a line's round caps at the first and last rows are drawn whole.
- [ ] A unit test in `plot.rs` (or `crates/gup-core/tests/builders.rs`) shows a
      line over x 2005..=2024 resolves to an x domain whose ends are within 1e-9
      of 2005 and 2024 before widening, and within one line width's worth of
      data units after it.

### AC2: The line and area builders use it

- [ ] `gup::line` and `gup::area` call `plot.fit_extent(0)` in their
      constructors. The y axes keep `nice()`.
- [ ] The policy survives `.x_scale(..)` (it is per axis, not per scale) and is
      bypassed by an explicit domain, as `tick_ends` is; both are tested.
- [ ] `Plot`'s own `line` and `area` adders are unchanged (see Decisions).

### AC3: Pinned domains removed where they only worked around this

- [ ] `examples/gallery/area_stacked.rs` drops
      `.x_scale(Linear::new().domain(2005.0, 2024.0))`; its golden
      `tests/golden/gallery/area_stacked.png` is re-blessed and still shows the
      stack starting at the left edge at 2005 and ending at the right edge
      at 2024.
- [ ] `examples/gallery/dark_multiline.rs` keeps its `domain(0.0, 24.0)`: a
      whole day is deliberate there, not a workaround.
- [ ] `git grep -n "domain(" -- examples docs/tutorials tests/tutorials.rs tests/gallery.rs`
      lists no other pin on a line or area x axis that exists only to avoid the
      outward rounding; any found is removed and its golden re-blessed.

### AC4: Every changed image read by eye

- [ ] Every golden of a line or area builder that changes is re-blessed and
      read: at least `gup_core/builder_line`, `gup_core/builder_area`,
      `gup_core/builder_area_stacked`, `gup_core/line_series`,
      `gup_core/line_monotone`, `gup_core/line_steps`, the tutorial images under
      `tests/golden/tutorials/` that show lines or areas, and
      `gallery/line_monotone`, `gallery/area_stacked` and `gallery/hero` if it
      changes. In each, the first and last rows lie on the plot's left and right
      edges (within the widening for line caps), tick labels stay inside the
      plot's x range, and no tick label is clipped.
- [ ] Description tests (`crates/gup-core/tests/describe.rs`) that quote x tick
      labels of a line or area chart are updated to the new ticks.

## Decisions

- **Type**:
  `#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)] enum AxisFit { #[default] Nice, Ticks, Extent }`,
  crate-private, in `plot.rs`. Field `axis_fit: [AxisFit; 3]` replaces
  `tick_ends`; every read of `tick_ends[i]` becomes
  `axis_fit[i] == AxisFit::Ticks`. Reason: one field, three exclusive policies,
  no boolean pair to keep consistent.
- **Setter**: `pub(crate) fn fit_extent(&mut self, axis: usize) -> &mut Self`,
  bumping `revision` like `round_to_ticks`. Not public: the policy is a
  builder's choice for its series axis; a `Plot` user who wants the extent sets
  the domain.
- **Zero-width extent**: read the domain through
  `DynPositionScale::current_domain`; when `lo == hi`, call `nice()` (its
  padding rule), else leave it.
- **Scope**: the x axis of `gup::line` and `gup::area` only. Not the scatter
  (points need the margin), not bars or histograms (GUP-561 keeps them on
  ticks), not the heatmap or box plot (band axes), not `Plot::line`/`Plot::area`
  (a `Plot` has no notion of a series axis), and not y axes.
- **Time scales**: the policy applies whatever scale the x axis holds; a `Time`
  x axis of a line chart ends at the first and last timestamps too.

## Escalation Triggers

- Removing `nice()` on the x axis leaves a line or area golden with no x tick
  label at all, or with ticks whose step changes between two resolves of the
  same chart at the same size.
- The widening for line caps does not happen on an extent axis (caps clipped at
  the plot edge), and making it happen needs changes to `widen` itself.
- A dogfood check (`./dogfood/run_all.sh`, t1 or t5) fails because of the new x
  domain.

## Out of Scope

- A public `Plot` setter for the fit policy (no story; ask if wanted).
- Curved or horizontal areas (deferred from N2).
- Tick placement changes beyond what the new domain implies.

## Technical Tasks

- [ ] Replace `tick_ends` with `axis_fit` and add `fit_extent`.
- [ ] Skip `nice`/`nice_to`/`round_out` for `Extent` axes, with the zero-width
      fallback.
- [ ] Call `fit_extent(0)` in `line` and `area`; add the AC1/AC2 tests.
- [ ] Remove the `area_stacked` pin; re-bless and read the goldens; update
      describe tests.
- [ ] `./dogfood/run_all.sh` (it uses `gup::line`).

## Dependencies

### Prerequisite Stories

- GUP-561: Axis Ends Fit Bars and Bands ✅ — the per-axis `tick_ends` policy
  this story generalises.
- GUP-547: Area Builder and Stacked Areas ✅ — the area builder.

## Testing Strategy

- **Unit tests**: domain ends of an extent axis; zero-width fallback; scale
  replacement and explicit domains.
- **Integration tests**: the line and area builder goldens and tutorial goldens.
- **Visual validation**: every re-blessed PNG read by eye (AC4).

## Success Metrics

- [ ] The gallery's stacked area has no pinned x domain and still fills the plot
      width.
- [ ] No line or area golden shows an empty strip between the plot edge and the
      first or last row.

## Risk Assessment

- **Medium**: many goldens move by a few pixels, and a wrong re-bless could hide
  a real regression. _Mitigation_: AC4 names what each image must show; read
  each, do not bless in bulk.
- **Low**: tick labels at the very ends may be thinned differently.
  _Mitigation_: the existing tick thinning runs after the domain is set; AC4
  checks that no label is clipped.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the goldens of AC4)
- [ ] `mask wasm-browser`: not needed; the browser scenes draw no line or area
      builder (checked with `grep` in `crates/gup-core/wasm-size/scatter/src`)
- [ ] WASM size: `mask wasm-size` scatter gz bytes (the plot.rs change is
      linked)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
