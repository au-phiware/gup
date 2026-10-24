---
implementer: sonnet
implementer-reason:
  palette order and label rule settled in Decisions; goldens re-blessed by a
  stated rule
touches:
  [
    crates/gup-core/src/scale/,
    crates/gup-core/src/marks.rs,
    crates/gup-core/wasm-size/scatter/index.html,
    crates/gup-core/tests/chain_legend.rs,
    tests/golden/gup_core/,
  ]
---

# GUP-392: Correct Visual Defaults

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: 🚧 In Progress
**Created**: 2026-10-04 **Revised**: 2026-10-24

## Context

**Revised 2026-10-24.** The 2026-10-04 version targeted the old `gup` path,
which was deleted in the RFC-001 flip (GUP-486, GUP-488); that text is in git.
Its four items were checked against `gup-core` on 2026-10-24, and all four hold:

| Item                         | Evidence in `gup-core` today                                                                                                                                                                                                                                                                                |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| One Okabe-Ito data source    | `OKABE_ITO` (`crates/gup-core/src/scale/categorical.rs:24`) is the only palette; `Categorical::okabe_ito()` extends it to 64 colours. The one leftover copy: `marks.rs` writes `Color::hex(0x0072b2)` (Okabe-Ito blue) five times as the marks' default colour.                                             |
| Inter, with its OFL licence  | `gup-text` bundles Inter 4.1 Regular and SemiBold subsets (`crates/gup-text/src/font.rs:27,35`), with `crates/gup-text/fonts/Inter-OFL.txt` shipped by the package's `include`; SVG writes `font-family="Inter, sans-serif"`.                                                                               |
| Tick precision from the step | `format_with_step` (`crates/gup-core/src/scale/mod.rs:540`) gives as many decimals as the step needs; `Linear`, `Pow`, `Symlog` and legend ramps use it. Tests: `fractional_ticks_label_with_enough_decimals` (0–1 gives `0.0, 0.2, …`), `fractional_steps_do_not_accumulate_error` (`2500` with step 500). |
| Muted theme colours          | `Theme::default()` (`crates/gup-core/src/theme.rs:108`): axis and labels `#333333`, grid `#e5e5e5`, title `#111111`, tooltip border `#999999`.                                                                                                                                                              |

Reading the goldens (`tests/golden/gup_core/builder_line.png`, `builder_bars`,
`builder_scatter`, `categorical_species`, `axis_titles`, `hello_world`,
`line_series`, `symlog_signed`) shows defaults that mostly look finished: light
1 px gridlines, a dark-grey axis, 12/13/18 px Inter, even margins, no clipped
labels. Two gaps remain, and this story fixes only those:

1. **Yellow is the fourth colour.** `OKABE_ITO[3]` is `#f0e442`, whose contrast
   on white is 1.32:1. The other seven are 2.25:1 (orange) and up (blue 5.19:1).
   Any chart with four or more series shows it: "Delta" in `builder_scatter.png`
   (translucent dots that are hard to see) and "West" in `builder_bars.png`.
2. **Large numbers print without grouping.** `axis_titles.png` labels GDP as
   `10000 … 60000`, and `chain_legend.rs` expects a ramp labelled `10000000`
   through `40000000`. `Log` and `Symlog` already print `10k` and `1M`
   (`format_log_tick`), but the linear family does not group digits.

## Read First

- `crates/gup-core/src/scale/categorical.rs`: `OKABE_ITO` and its doc (lines
  14–33), `extend` (lines 188–230), test
  `codes_follow_the_palette_and_null_is_grey` (lines 343–356).
- `crates/gup-core/src/scale/mod.rs`: `MINUS`, `typographic`, `format_with_step`
  (lines 524–555); test `fractional_steps_do_not_accumulate_error` (lines
  628–634).
- The callers of `format_with_step`: `scale/linear.rs` (`ticks`, lines 187–197),
  `scale/pow.rs` (`ticks`, lines 214–225), `scale/symlog.rs` (`ticks`, the
  linear branch, lines 238–250), `scale/legend.rs` (`with_data_ticks`, lines
  130–160; `build`, lines 197–230).
- `crates/gup-core/src/marks.rs`: `DEFAULT_FILL` / `DEFAULT_COLOR` (lines 64–67,
  136–139, 194–197, 246–249) and the test literal at line 268.
- `crates/gup-core/wasm-size/scatter/index.html` lines 69–95 (the four expected
  Okabe-Ito colours).

## User Story

> "As a user who draws a chart without choosing colours or tick formats, I want
> every series visible on white and large numbers readable at a glance, so that
> the default chart is one I can publish."

## Acceptance Criteria

### AC1: Yellow and black last

- [ ] `OKABE_ITO` is, in this order: `0xe69f00` orange, `0x56b4e9` sky blue,
      `0x009e73` bluish green, `0x0072b2` blue, `0xd55e00` vermillion,
      `0xcc79a7` reddish purple, `0xf0e442` yellow, `0x000000` black. Its doc
      comment lists that order and says why: yellow has 1.32:1 contrast on
      white, so it comes after every colour with at least 2.25:1.
- [ ] A doctest on `OKABE_ITO` asserts `OKABE_ITO[3] == Circle::DEFAULT_FILL`
      and `OKABE_ITO[6] == Color::hex(0xf0e442)`.
- [ ] The generated colours do not change: a new unit test in `categorical.rs`
      pins `Categorical::okabe_ito()`'s LUT entries 8, 9 and 63 to their values
      on `main` before this story (read them first, then write the test).
- [ ] `codes_follow_the_palette_and_null_is_grey` checks
      `c.eval(3.0) == Circle::DEFAULT_FILL` (it checks `4.0` today).

### AC2: The marks' default colour comes from the palette

- [ ] `Circle::DEFAULT_FILL`, `Rect::DEFAULT_FILL`, `Segment::DEFAULT_COLOR` and
      `Line::DEFAULT_COLOR` are `OKABE_ITO[3]`; the test at `marks.rs:268` uses
      `Circle::DEFAULT_FILL`.
- [ ] `git grep -n "0x0072b2" crates/gup-core/src` prints only the `OKABE_ITO`
      entry.

### AC3: Thousands grouped from 10,000

- [ ] A tick set whose largest absolute value is at least 10,000 labels every
      value with `,` between groups of three integer digits; smaller sets are
      unchanged. Unit tests on `format_with_step` cover: `(12345.0, 5.0, true)`
      → `12,345`; `(-1234567.0, 1.0, true)` → `−1,234,567` (typographic minus);
      `(2500.0, 500.0, true)` → `2,500`; `(2500.0, 500.0, false)` → `2500`;
      `(10000.5, 0.5, true)` → `10,000.5`; `(0.0, 5000.0, true)` → `0`;
      `(2019.0, 1.0, false)` → `2019`.
- [ ] `Linear::new().domain(0.0, 60000.0).ticks(6).labels` is
      `["0", "10,000", "20,000", "30,000", "40,000", "50,000", "60,000"]`, and
      `Linear::new().domain(2000.0, 2024.0).ticks(5).labels` is
      `["2000", "2005", "2010", "2015", "2020"]` (unit tests in `linear.rs`).
- [ ] `chain_legend.rs` expects
      `["0", "10,000,000", "20,000,000", "30,000,000",     "40,000,000"]` and
      `"10,000,000"` at line 161.
- [ ] A user's `tick_format` still wins: `tests/tick_format.rs` and its goldens
      (`tick_format_currency`, `tick_format_percent`, `tick_format_date`) pass
      without a re-bless.

### AC4: Goldens and scenes updated, by eye

- [ ] `GUP_BLESS=1 scripts/with_cargo_env.sh cargo test -p gup-core --all-features -- --test-threads=1`
      re-blesses the goldens; the retrospective lists every changed file under
      `tests/golden/gup_core/` with its reason: "4+ categorical keys" or "a
      label of 10,000 or more". Any other changed golden is an escalation.
- [ ] Read `builder_scatter.png`: the fourth series ("Delta") is blue
      (`#0072b2`), as is its legend swatch; no yellow dots remain.
- [ ] Read `builder_bars.png`: "West" is blue in both panels and the legend.
- [ ] Read `axis_titles.png`: the x labels read `0, 10,000, … 60,000`, adjacent
      labels do not touch, and the axis title sits below them without overlap.
- [ ] `crates/gup-core/wasm-size/scatter/index.html`'s fourth expected colour is
      `[0x00, 0x72, 0xb2]` and its comment still says "the first four Okabe-Ito
      colours"; `mask wasm-browser` passes.

### AC5: No other user of the old order

- [ ] `git grep -n -i "f0e442\|0xf0, 0xe4" -- ':!docs'` prints only the
      `OKABE_ITO` entry and the new doctest. Tests that index the palette
      (`OKABE_ITO[i]` in `builders.rs`, `plot.rs`, `ordinal_png.rs`) name the
      constant, not a hue, and pass unchanged.
- [ ] `mask dogfood` reports the same PASS/XFAIL/FAIL counts as before the
      change (10 PASS, 0 XFAIL, 0 FAIL on 2026-10-24).

## Decisions

- **Reorder rather than replace.** Keep all eight Okabe-Ito colours (the October
  review's Decision 4); move yellow to index 6, before black. Blue moves to
  index 3, so a four-series chart ends on the marks' default colour, which is
  the palette's highest-contrast hue.
- **One constant.** Reorder `OKABE_ITO` itself; do not add a second ordering.
  `extend` measures distance to the whole set, so the generated colours (index 8
  and up) do not depend on the order (AC1 pins this).
- **Grouping rule, per tick set.** Group when
  `values.iter().any(|v| v.abs() >= 10_000.0)`. The threshold keeps four-digit
  values such as years on a `Linear` axis (`2019`) ungrouped, and deciding per
  set keeps one axis consistent (`8,000` beside `10,000`, never `8000`).
- **Signatures** (both `pub(crate)` in `scale/mod.rs`):
  `fn groups_thousands(values: &[f64]) -> bool` and
  `fn format_with_step(v: f64, step: f64, grouped: bool) -> String`. Every
  caller computes `grouped` once from its full value list before labelling;
  `legend.rs`'s `with_data_ticks` computes it before `values` is consumed.
- **Separator `,`** (U+002C, in the bundled subset); decimals keep `.`. Locales
  are the user's `tick_format`.
- **Out of the rule:** `Log`, `Symlog`'s decade ticks (`format_log_tick`, which
  already prints `10k`), `Time`, `Band`, `Point` and a user's `tick_format`.
- **No theme change.** `Theme::default()` values, sizes and margins stay; the
  goldens show them working.

## Escalation Triggers

- A golden changes for a reason other than the two in AC4, or a generated
  palette colour (index 8 and up) changes.
- A y axis with grouped labels pushes the plot rect so far that a test with a
  layout assertion (`theme_png.rs`, `builders.rs`) fails other than by a
  re-blessable pixel diff.
- Any dogfood check changes status.

## Out of Scope

- A dark theme, subtitles and font-weight changes: N2 (subtitle and dark theme).
- Tabular figures and kerning: GUP-405. Non-Latin fallback: GUP-416.
- Unicode thin-space or locale-aware grouping: a user's `tick_format`.
- A high-contrast theme: N4.

## Technical Tasks

- [ ] Read LUT entries 8, 9, 63 on unchanged code; write the pinning test.
- [ ] Reorder `OKABE_ITO`, update its doc and add the doctest.
- [ ] Point the four `DEFAULT_*` constants and the `marks.rs:268` test at the
      palette.
- [ ] Add `groups_thousands`, extend `format_with_step`, update the five call
      sites and the existing tests; add the AC3 tests.
- [ ] Update `chain_legend.rs` and `index.html`.
- [ ] Re-bless, read the four named PNGs, list every changed golden.
- [ ] Run `mask wasm-browser` and `mask dogfood`.

## Dependencies

### Prerequisite Stories

- GUP-388: Visual Regression Harness ✅ — the bless workflow.
- GUP-474: tick formats ✅ — the `tick_format` override that must still win.

### Enables Stories

- None.

## Testing Strategy

- **Unit tests**: `format_with_step` cases; `Linear` label sets; palette order
  and the pinned generated colours.
- **Integration tests**: `chain_legend.rs`; every golden test with `GUP_BLESS`
  unset after the re-bless.
- **Visual validation**: the four PNGs in AC4, read with the Read tool.
- **Browser**: `mask wasm-browser`.

## Success Metrics

- [ ] No default series colour below 2.25:1 contrast on white until the seventh
      series.
- [ ] No default linear-family label of 10,000 or more without grouping.

## Risk Assessment

- **Low**: wider grouped y labels shift the plot rect by a few pixels in charts
  with large y values. _Mitigation_: the goldens catch it; layout already
  measures label widths.
- **Low**: a test elsewhere indexes the palette by number with a colour in mind.
  _Mitigation_: AC5's grep.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read)
- [ ] `mask wasm-browser`: required (a browser scene's expected colours change)
- [ ] WASM size: `mask wasm-size` scatter gz bytes
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Blocked (2026-10-24)

**Question**: grouped labels ("10,000", ...) add comma glyphs, so the guide
instance upload in `crates/gup-core/tests/zoom_uploads.rs` grows from the
recorded 1,400,736 B to 1,503,048 B over 300 frames (+102,312 B, +7.3%). Two
tests fail on it. The test says to record the new total there and in
`PERF_BUDGETS.md`; both are outside this story's `touches:` list.

**Options**: (1) the orchestrator re-records `instance_bytes`
(`zoom_uploads.rs:51,66`) and `PERF_BUDGETS.md` to 1_503_048 (measured, a direct
result of AC3); (2) drop grouping (contradicts AC3).

**Recommendation**: option 1.

Done and committed (WIP): palette, marks, grouping, tests, 13 goldens re-blessed
and read. `mask wasm-browser` fails here with "adapter none (no WebGPU)", an
environment limit, so the `index.html` colour is untested in a browser. Other
gup-core tests and dogfood pass.
