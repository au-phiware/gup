---
implementer: opus
implementer-reason:
  attribution across the whole crate decides which cuts are safe; each candidate
  needs judgement about what a browser scatter may leave unlinked without a
  behaviour change (sonnet could apply the cuts once chosen)
touches:
  [
    crates/gup-core/PERF_BUDGETS.md,
    crates/gup-core/Cargo.toml,
    crates/gup-core/src/lod.rs,
    crates/gup-core/src/lod/,
    crates/gup-core/src/selection.rs,
    crates/gup-core/wasm-size/,
    scripts/wasm_size.sh,
  ]
---

# GUP-744: The Browser Build Stays Well Under Its Size Cap

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 🚧 In Progress
**Created**: 2026-11-19

## Context

RFC-001 §12 risk 10 caps gup-core at 400,000 B gz over bare wgpu
(`wasm.over_wgpu.gz_bytes`, `scripts/wasm_size.sh`, enforced by CI). GUP-451 cut
it from 356,914 to 263,544 B in October. Since then it has grown to 363,801 B
(scatter 396,161 B gz, bare wgpu 32,360), 36 KB of headroom, at roughly 8 KB a
week: level of detail (GUP-714 to 721, `Lod::Auto` linking the tier build, about
+6,000), accessibility (GUP-619 to 626), columnar input (GUP-628, 629, 664) and
interaction (GUP-676, 684, 685). Four more weeks at that rate breaks the cap
before release 0.1. Each re-recording so far has moved the scatter row, never
the cap, and nothing says when a story must trim instead.

`PERF_BUDGETS.md` "Where the bytes go (GUP-450)" is the last attribution (raw
bytes, 2026-10-18, 1.4 MB file); it predates all of the growth above. The
harness already builds with `opt-level = "z"`, LTO and one codegen unit, and
without the `svg` feature (`svg-export` is a separate +4,057 B row). GUP-716
found `prepare` copies generated per row type. This story re-attributes, cuts
what a browser scatter does not need, and writes the headroom policy.

## Read First

- `crates/gup-core/PERF_BUDGETS.md`: the WASM rows (lines 90 to 94), "WASM size"
  (from line 191), "Where the bytes go (GUP-450)" (from about line 244), and the
  re-record notes for GUP-451, GUP-715, GUP-718 and GUP-720 (lines 436 to 460
  and 575 to 630).
- `maskfile.md` `wasm-attrib` (line 112) and `scripts/wasm_attrib.sh`
  (`--row-type`); `scripts/wasm_size.sh`.
- `crates/gup-core/Cargo.toml` `[features]` (line 71: `svg`, `window`, `web`,
  `arrow`) and `crates/gup-core/wasm-size/scatter/Cargo.toml`.
- `docs/planning/stories/GUP-716_*.md` retrospective, the `prepare` copies.

## User Story

> "As a developer shipping a gup chart to the browser, I want the library to
> stay far enough under its size cap that new features do not force a choice
> between the cap and the feature, so that release 0.1 ships inside it."

## Acceptance Criteria

### AC1: Attribute the current scatter

- [ ] Run `mask wasm-size` and `mask wasm-attrib` on current `main`; paste into
      the story: the crate table (top ten, raw B and share), the 20 largest
      monomorphised groups with instantiation counts, and the 30 largest
      functions and data segments.
- [ ] Group `gup_core`'s bytes by module (`lod`, `selection`, `plot`, `scale`,
      `shader`, `describe`, `tooltip`, `gpu_pick`, `text_layer`, `input`, `web`,
      others) using the function names, and list each cut candidate with its
      measured gz saving (build with the candidate removed, `mask wasm-size`
      before and after, one candidate at a time).
- [ ] Record the Debug/Display/panic-string share (`core::fmt`, `Debug` impls,
      `unwrap`/`expect` messages) and whether `wgpu` features or the harness's
      `wasm-bindgen`/`web-sys` features pull in unused code.

### AC2: Cut

- [ ] Over bare wgpu falls by at least **25,000 B gz** from the AC1 baseline
      (about 363,801 to at most 338,800). If AC1 shows safe candidates sum to
      less, the target is their sum with a **floor of 15,000 B**; below the
      floor, stop and report (Escalation).
- [ ] No behaviour change: every native golden is byte-identical (no `BLESS`),
      `cargo test -- --test-threads=1` passes without test edits, and
      `mask wasm-browser` passes with identical pixel counts.
- [ ] Public API unchanged: `cargo check --examples` and the doctests pass
      without edits, and `cargo public-api`-style diff, or a read of the `pub`
      items in the diff, shows no signature change.
- [ ] A cut that makes code unreachable by default but available by a feature or
      by an opt-in is not allowed unless the default behaviour is unchanged for
      the scatter and for `Lod::Auto` on 10M rows (the LOD tests pass unedited).

### AC3: Headroom policy

- [ ] `PERF_BUDGETS.md` gains a "Headroom policy" paragraph under "WASM size"
      with the rule in Decisions, the AC1 attribution (condensed, dated, with
      commit) replacing or sitting beside the GUP-450 table, and the cuts made
      with their measured gz.
- [ ] `scripts/perf_budget.sh` still parses the file (`mask perf-budget` runs
      and prints the WASM rows), and no budget row value is edited by the worker
      (the orchestrator re-records).

## Decisions

- **Target**: at least 25,000 B gz off `wasm.over_wgpu` (floor 15,000), set to
  reach under 340,000 so that headroom is at least 60 KB, about seven weeks at
  the recent rate. Measured with `mask wasm-size` before and after on the same
  toolchain.
- **Policy to write** (change the numbers only by an AC1 finding):
  - Until 0.1 ships, `wasm.over_wgpu.gz_bytes` stays at most **360,000** (40 KB
    under the cap). A story may re-record the scatter row while over_wgpu is at
    most 360,000.
  - A story whose change takes over_wgpu above 360,000 trims at least its own
    growth in the same story, or records an owner-approved exception in its
    retrospective.
  - Above **380,000** no story merges without a trim story ahead of it; above
    the 400,000 cap CI fails, as now.
  - A story that adds more than 5,000 B gz states in its retrospective what
    linking it costs a scatter that does not use it.
- **Candidates, in the order to measure** (none is pre-approved; AC1 decides):
  generic code copied per row type (`prepare`, `Selection<T>` paths; GUP-716);
  the LOD tier build and size-ordered LOD paths that a layer under 200,000 rows
  never reaches (only if a runtime-size branch already exists, so that removing
  the link would need a feature: that is an escalation, not a cut);
  `Debug`/`Display` impls and `expect` strings (shorter messages are allowed,
  messages asserted by tests are not touched); the edge-solve reach index;
  text-shaping tables and font data; `describe`/tooltip code behind a branch the
  scatter never takes; wgpu features and limits requested but unused.
- Cut by restructuring (non-generic inner functions, `#[inline(never)]` on cold
  paths, `dyn` where the call is not hot) before cutting by feature.
- Build flags: `panic = "abort"`, `wasm-opt -Oz` and `strip` are tried only if
  the harness does not already use them; each counts as one candidate, and the
  flags must be ones `web.rs` can document for users.
- The cap and the `+2%` scatter tolerance stay as they are.

## Escalation Triggers

- A win needs a public API change, or removes or gates a user-visible feature
  (LOD, accessibility, hover, tooltips, columnar input).
- Safe candidates sum to less than 15,000 B gz.
- Anything in AC1 suggests the 400,000 cap itself should move, or that the
  harness (not the library) is mismeasuring.
- A cut changes a golden image, a browser pixel count or `Lod::Auto`'s default
  choice.
- A cut conflicts with a story in flight on `selection.rs` or `lod/`.

## Out of Scope

- Raising or questioning the 400,000 cap (owner decision).
- Re-recording budget rows in `PERF_BUDGETS.md` (the orchestrator does).
- New features behind flags to make the browser build smaller; a `lod` or `a11y`
  feature split needs its own story.
- Native binary size, SVG export (`svg-export`, GUP-557), compile time.
- Shrinking the Inter font subsets again (GUP-451 did the ASCII SemiBold cut).

## Technical Tasks

- [ ] Baseline `mask wasm-size` and `mask wasm-attrib`; paste (AC1).
- [ ] Group by module and measure each candidate alone (AC1).
- [ ] Apply cuts, largest first, measuring after each; keep a cut only if it
      saves at least 1,000 B gz.
- [ ] Run tests, goldens, `mask wasm-browser` (AC2).
- [ ] Write the policy and the attribution into `PERF_BUDGETS.md` (AC3).
- [ ] Propose the new re-record values in the retrospective for the
      orchestrator.

## Dependencies

### Prerequisite Stories

- GUP-721: LOD for Size-Ordered Layers ✅ (the last large linker of new code).
- GUP-450 ✅ and GUP-451 ✅: the attribution tooling and the earlier cuts.

### Enables Stories

- Release 0.1 (Track N3): a size budget that tolerates the remaining stories.

## Testing Strategy

- **Unit and integration tests**: the existing suite, unedited.
- **Visual validation**: native goldens byte-identical; `mask wasm-browser`
  pixel counts identical (before and after both pasted).
- **Performance**: `mask wasm-size` before and after on one toolchain;
  `mask perf-budget` shows no CPU or GPU row worse than its tolerance (cuts must
  not de-inline a hot path).

## Success Metrics

- [ ] `wasm.over_wgpu.gz_bytes` at most 338,800 (or the AC1-justified target,
      never above 348,800).
- [ ] Headroom policy present in `PERF_BUDGETS.md` and followed by this story.

## Risk Assessment

- **Medium**: size cuts that de-inline hot paths or box generics cost CPU.
  _Mitigation_: `mask perf-budget` before and after; revert a cut that moves a
  row past tolerance.
- **Medium**: the savings are smaller than 25 KB because the growth is real
  code. _Mitigation_: the 15 KB floor and the escalation; the policy still
  lands.
- **Low**: another story lands size growth while this one runs. _Mitigation_:
  measure before and after on one base commit; the orchestrator re-records on
  the merge.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified: goldens byte-identical, `mask wasm-browser`
      pixel counts identical to before
- [ ] `mask wasm-browser` passes (wasm code is touched)
- [ ] WASM size: `mask wasm-size` before and after pasted
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
