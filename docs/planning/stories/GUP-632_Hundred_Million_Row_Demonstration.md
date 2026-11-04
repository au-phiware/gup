---
implementer: sonnet
implementer-reason:
  an example and measured runs over APIs that exist by then; the README wording
  follows a stated rule, and budgets are recorded by the orchestrator
touches:
  [
    crates/gup-core/examples/hundred_million.rs,
    crates/gup-core/Cargo.toml,
    README.md,
    CHANGELOG.md,
  ]
---

# GUP-632: Hundred-Million-Row Demonstration

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 📋 Planned
**Created**: 2026-11-04

## Context

The November review: "a 100M-row and then 1B-row demonstration with budgets.
Only after this may the README say 'billion'." The README's "Not yet" list says
"Nothing above 10M rows is measured, and there is no level of detail (N5)." By
the time this story starts, GUP-628 (columnar layers) and GUP-629 (key columns)
let 100M rows in without a row type, and GUP-631 has chosen a level of detail
design whose follow-up stories build it. This story is the proof: a runnable
example a user can try, the measured numbers on the development machine, and
README wording that says exactly what was measured.

## Read First

- The retrospectives of GUP-627, GUP-628 and GUP-631, and the RFC-001 "GUP-628
  findings" and "GUP-631 findings" sections (the API to use and the LOD switch,
  if any).
- `crates/gup-core/examples/zoom_bench.rs` lines 1–80 (window, timing, `metric`
  lines).
- `README.md` lines 1–60 (tagline, "Why Gup", "Not yet").
- `crates/gup-core/PERF_BUDGETS.md` lines 20–60 and the "History" section (how
  budgets are recorded; the worker does not edit it).

## User Story

> "As a prospective user with large data, I want to run a 100M-point chart
> myself and read what it costs, so that I can trust Gup's scale claims."

## Acceptance Criteria

### AC1: The example

- [ ] `crates/gup-core/examples/hundred_million.rs` (feature `window`): 100M
      rows by default (`--rows N`), built from `Columns` (f32 x and y, a 5-label
      key column for colour), opened in a window with wheel zoom, drag pan and
      keyboard keys; prints `metric scale100m.<name> <value>` lines for
      first-frame time, peak RSS, and over a scripted 600-frame zoom (as
      `zoom_bench`), GPU pass median and p95 and CPU median.
- [ ] With `GUP_SCREENSHOT_PATH=<path>` it renders one frame offscreen to a PNG
      and exits (the gallery examples' convention,
      `scripts/generate_gallery.sh`).
- [ ] It uses the level of detail of GUP-631's follow-up stories if they have
      landed; if they have not, it draws every visible row and says so in its
      doc comment and output.

### AC2: Measured

- [ ] Release runs on the development machine (nothing else running): 10M, 30M
      and 100M rows, three runs each; a table in the retrospective with every
      metric, and the PNG at 100M read by eye (marks present across the plot,
      axes and legend correct).
- [ ] The orchestrator is given the proposed `PERF_BUDGETS.md` rows (names,
      values, checks: `info` for timings, exact for counts) in the
      retrospective; the worker does not edit `PERF_BUDGETS.md`.

### AC3: Honest README

- [ ] README wording follows the rule in Decisions, with the figures and a link
      to the example.
- [ ] The "Not yet" line about 10M rows is replaced by what is still not
      measured (1B rows), or removed if 1B was measured elsewhere.
- [ ] A CHANGELOG `[Unreleased]` entry names the example and the figures.

## Decisions

- **README rule**: the README may say "100 million points" in a sentence that
  states the machine and the measured GPU pass median, and may call it
  "interactive" only if the 100M zoom GPU pass median is at most 16.7 ms and
  first frame at most 10 s on the development machine. Otherwise it states the
  numbers without "interactive". The word "billion" stays out until a 1B-row run
  is measured (not this story).
- **Data**: generated in the example (seeded), sorted by x so chunk culling and
  LOD tiers have coherent chunks; documented as the favourable case, with one
  `--shuffle` run in the table for honesty.
- **Not a gate**: the example is not in CI (memory and time);
  `cargo check --examples` covers its compilation.

## Escalation Triggers

- The 100M run does not fit the development machine's memory even with
  `Retain::GpuOnly` and columns.
- GUP-631 chose LOD but its implementation stories have not landed: report
  before measuring, as the orchestrator may prefer to wait.

## Out of Scope

- A 1B-row run (needs out-of-core paging; a later story).
- LOD itself (GUP-631's follow-ups).

## Technical Tasks

- [ ] The example, its `[[example]]` entry with `required-features`.
- [ ] Runs, table, PNG, proposed budget rows.
- [ ] README and CHANGELOG by the rule.

## Dependencies

### Prerequisite Stories

- GUP-628: Columnar Layers 📋; GUP-629: Key Columns 📋.
- GUP-631: Level of Detail Spike 📋, and the implementation stories it lists
  (soft: AC1 says what happens without them).

## Testing Strategy

- **Performance**: AC2 runs.
- **Visual validation**: the 100M PNG.

## Success Metrics

- [ ] Anyone can reproduce the README's scale figure with one command.

## Risk Assessment

- **Medium**: the numbers may disappoint. _Mitigation_: the README rule turns
  any result into an honest sentence.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` (the README doctest)
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the 100M PNG)
- [ ] `mask wasm-browser`: not needed, a native example
- [ ] WASM size: unchanged: no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
