---
implementer: sonnet
implementer-reason:
  the cause is attributed below; the harness change, the one cut, the diff tool
  and the budget rule are fixed in Decisions
---

# GUP-459: Per-Row-Type WASM Cost: Measure It Cleanly, Cut It and Budget It

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-18

## Context

`wasm.per_row_type.gz_bytes` estimates what one more `Selection` row type costs
a WASM program. `scripts/wasm_size.sh` builds the scatter harness with and
without its `extra-row-type` feature and takes the difference (GUP-450). The
number went 941 (GUP-451) → 1,598 (the S9a–S9d merge) → 2,569 (GUP-456).
GUP-456's worker stubbed its new `Layer` methods and the number did not move.
The row is `info`, so the growth fails nothing.

The scribe re-measured on 2026-10-18 at `b3f87b5`: 312,441 B gz without the
feature and 315,010 with it, so 2,569 B gz (5,808 B raw). Diffing `twiggy top`
of the two builds by item, with hashes and `.NNNN` suffixes stripped, gives
5,144 B raw outside the name section:

| Raw B | Item (group)                                                             | What it is                                                                                                                                               |
| ----: | ------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2,510 | `wasm_bindgen_futures::future_to_promise::{{closure}}::{{closure}}` (+1) | the harness's own `async fn render_extra_row_type` state machine, which inlines resolve, pick and render; it grows whenever the library they reach grows |
|   277 | `future_to_promise::{{closure}}` (+1), plus ~240 of its drops and shims  | the same async export                                                                                                                                    |
|   464 | `<Resolved as Clone>::clone` (+1), −534 `InteractiveScatter::resolved`   | an inlining shift caused by the second caller                                                                                                            |
|   802 | `<Selection<T,M> as Layer>::bind_axes`                                   | library, per row type                                                                                                                                    |
|   234 | `Layer::tooltip_text`                                                    | library, per row type                                                                                                                                    |
|   204 | `AxisEncoding<T>::evaluate`                                              | library, per row type                                                                                                                                    |
|   151 | `Layer::dispatch`                                                        | library, per row type                                                                                                                                    |
|  ~300 | `row_any`, `as_any_mut`, `type_id`, vtable shims, closures               | library and the harness's closures                                                                                                                       |

**Cause.** About 3.3 KB of the 5.1 KB raw delta is the measuring function, not
the library. The library's per-row-type code is about 1.6 KB raw, and
`bind_axes` is half of it. `prepare`, `fit_domains` and `hit_items` do not
appear: their instantiations are identical across row types, and LTO merges
them. So the metric drifted with the harness's async body, and that is why
stubbing methods did not move it.

## Read First

- This story's Context table: the attribution is done.
- `scripts/wasm_size.sh` (whole, ~110 lines) and `scripts/wasm_attrib.sh`
  (whole, ~75 lines).
- `crates/gup-core/wasm-size/scatter/src/lib.rs` lines 1095–1143
  (`render_extra_row_type`).
- `crates/gup-core/src/selection.rs` `bind_axes` (lines 638–708).
- `crates/gup-core/PERF_BUDGETS.md` "Where the bytes go (GUP-450)" (last
  paragraph) and the `wasm.per_row_type` history entries.

## User Story

> "As the maintainer, I want the per-row-type WASM cost to measure only library
> code and to fail CI when it grows, so that monomorphisation creep is caught
> when it happens rather than discovered later."

## Acceptance Criteria

### AC1: A clean measurement

- [ ] `render_extra_row_type` becomes the synchronous export in Decisions. No
      async, resolve or render runs on the extra row type.
- [ ] `mask wasm-size` prints `wasm.per_row_type.gz_bytes`. Record the value
      (M₀).
- [ ] The diff tool (AC3) shows instantiations for the extra row type of at
      least `bind_axes`, `tooltip_text` and `dispatch`. That proves the vtable
      was linked. Without them, escalate.

### AC2: Cut `bind_axes`

- [ ] `Selection::bind_axes` keeps only the row-type-dependent step (building
      each `AxisEncoding<T>` from its stored closure). Everything else moves
      into a non-generic `fn axis_scale_for(...)` in `selection.rs` (Decisions):
      the role match, the channel name formatting, both error messages and the
      `reads_keys` check.
- [ ] `tests/plot_closures.rs` passes unchanged, including both error-message
      tests.
- [ ] Re-measure: the value is M. In the diff tool, `bind_axes` for the extra
      row type is at most 400 B raw. If it is not, record the figure in the
      Retrospective and continue; this is not an escalation.

### AC3: A reusable diff

- [ ] `scripts/wasm_attrib.sh --row-type` builds the harness with and without
      `extra-row-type`. It prints `== row type`: per-group raw byte and
      instantiation-count differences, with hashes and `.NNNN` suffixes
      stripped, the 30 largest by absolute size, and the total. The existing
      output without the flag is unchanged.
- [ ] `maskfile.md`'s `wasm-attrib` entry documents the flag.

### AC4: A real budget

- [ ] The `wasm.per_row_type.gz_bytes` row in `PERF_BUDGETS.md` records M, and
      its check is `<= B` with B = ⌈1.5 · M / 100⌉ · 100.
- [ ] Seeded proof: add a temporary 4 KB `#[inline(never)]` generic function
      called from `Selection::tooltip_text`. `mask wasm-size` exits non-zero,
      naming `wasm.per_row_type.gz_bytes`. Revert the seed and record both runs
      in the Retrospective.
- [ ] `PERF_BUDGETS.md` gains a History entry with this story's Context table,
      M₀, M and B.

## Decisions

- **Harness export**: replace the async `render_extra_row_type` with

  ```rust
  #[cfg(feature = "extra-row-type")]
  #[wasm_bindgen]
  pub fn extra_row_type() -> usize {
      let mut plot = Plot::new();
      // The same layer as before: circles over `Extra` with x, y, a tooltip
      // and an `on(Event::Click, ..)` handler.
      plot.circles(extra_rows()).x(|r| r.a).y(|r| r.b) /* … */;
      let chart: Box<dyn Chart> = Box::new(plot);
      std::hint::black_box(chart).title_text().map_or(0, str::len)
  }
  ```

  Boxing the `Selection` as `dyn Layer` (in `Plot::add`) links every `Layer`
  method through the vtable, and `black_box` keeps the plot alive. `index.html`
  never called the old export.

- **Cut**: a non-generic function that returns the scale or the existing errors,
  word for word. `bind_axes` loops over its closures, calls it and wraps the
  result:

  ```rust
  fn axis_scale_for<'s>(
      mark: &'static str,
      desc: &ChannelDesc,
      x: &'s dyn DynPositionScale,
      y: &'s dyn DynPositionScale,
  ) -> Result<&'s dyn DynPositionScale>;
  ```

- **Budget rule**: B = ⌈1.5 · M / 100⌉ · 100. It is an absolute ceiling, not a
  percentage, because the number is small and inlining moves it by a few hundred
  bytes. 50% of headroom catches a new per-row-type method of a few hundred
  bytes gz without failing on noise. CI already checks every `wasm.*` row
  through `perf_budget.sh --wasm-only`.
- The diff logic of AC3 is the Python the scribe used (group by name with
  `::h[0-9a-f]{16}(\.\d+)?$` stripped), inside `wasm_attrib.sh`'s existing
  `python3 -I` heredoc style.

## Escalation Triggers

- After AC1, M₀ is under 300 B gz (the vtable was probably not linked) or over
  1,600 B gz.
- The `bind_axes` split changes any error message or any test result.
- `wasm.scatter.gz_bytes` (the default build) moves by more than 1%.

## Out of Scope

- Moving other `Layer` bodies out of the generics (GUP-451's skipped AC3). Do it
  only if a later story's growth breaks the new budget.
- Builders' (S10) per-row-type cost: they are not in the harness.

## Technical Tasks

- [ ] Harness export (AC1); measure M₀.
- [ ] `axis_scale_for` and the slimmer `bind_axes` (AC2); measure M.
- [ ] `wasm_attrib.sh --row-type`; maskfile docs (AC3).
- [ ] Budget row, seeded proof, History entry (AC4).

## Dependencies

### Prerequisite Stories

- GUP-450: WASM Size Attribution and Row-Type Gate ✅ — the metric and
  `mask wasm-attrib`.
- GUP-456: RFC-001 S9e ✅ — the build the 2,569 B figure came from.

### Enables Stories

- Every later gup-core story: per-row-type growth fails CI instead of drifting.

## Testing Strategy

- **Measurement**: `mask wasm-size` before and after each change;
  `wasm_attrib.sh --row-type` to attribute.
- **Unit/integration**: `tests/plot_closures.rs` for the `bind_axes` split.
- **Gate proof**: the seeded 4 KB function fails `mask wasm-size`.

## Success Metrics

- [ ] The metric measures library code only, and fails CI on growth past B.
- [ ] The cause of the 941 → 2,569 drift is recorded in `PERF_BUDGETS.md`.

## Risk Assessment

- **Low**: `black_box` alone might not keep every vtable method. _Mitigation_:
  AC1 checks the instantiations in the diff and escalates.
- **Low**: the budget fails on noise. _Mitigation_: 50% headroom, re-recorded in
  the same commit as any legitimate growth.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] `mask wasm-size` and `mask wasm-browser` pass
- [ ] `mask old-path-loc` recorded (expected unchanged, 28,910)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
