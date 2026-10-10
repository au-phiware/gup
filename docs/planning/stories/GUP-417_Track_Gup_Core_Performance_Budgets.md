# GUP-417: Track gup-core Performance Budgets

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-10

## Context

[GUP-396](GUP-396_Gup_Core_Vertical_Slice_Window_Performance.md) (S0b) measured
`gup-core`'s 100K-point zoom benchmark (`zoom_bench`, RFC-001's S0 exit
criterion 3) at a GPU render-pass median of 2.7–3.0 ms on the development
machine (Intel HD Graphics 630, Mesa 26.0.0 Vulkan, 1920×1080 60.02 Hz, rustc
1.93.1). [GUP-414](GUP-414_RFC_001_S4a_Column_Store_Chunking_Append.md) (S4a)
ran the same benchmark, on the same machine, against both its own tree and the
immediately preceding tree (`b176400`, which already includes
[GUP-407](GUP-407_Subset_Inter_And_SVG_Font_Embedding.md)'s font subset), and
got about 3.7 ms on both — a ~25–35% regression against S0b that predates S4a
itself, caught only because that story happened to run a before/after comparison
for an unrelated reason. Nobody had been watching the number between S0b and
S4a, so the regression sat unnoticed through S1, S2, S3, and
GUP-406/GUP-407/GUP-410.

GUP-414's retrospective named "GUP-407's text changes or the machine's state" as
candidates and left the question open, without considering the change that sits
chronologically between S0b and GUP-414's own baseline and that most directly
touches the GPU render pass:
[GUP-401](GUP-401_RFC_001_S3_Scene_Renderer_RenderTarget.md) (S3) made 4× MSAA
(`DEFAULT_SAMPLES` in `crates/gup-core/src/target.rs`) the default sample count
for every Gup-owned target, including `WindowTarget`. S3's own retrospective
measured MSAA's _visual_ effect (it visibly smooths diagonal rules and
fractional-coordinate rects; circles antialias analytically and are unaffected
to within 2/255) but never re-ran `zoom_bench` to measure its _GPU time_ cost at
100K points. Turning on 4× multisampling plus a resolve pass is exactly the kind
of change that would cost a per-frame GPU render-pass fraction of a millisecond,
which is the right order of magnitude for the gap S4a found. This story's first
job is to settle that, with a measurement, not a guess.

More generally, `gup-core` already has four separate, working measurement tools
— `examples/zoom_bench.rs` (GPU pass / CPU / uploads per frame),
`selection::tests::pipeline_timings` (pipeline compose+create time,
`--ignored --nocapture`), `mask wasm-size` (gzipped WASM size), and the
`Upload`/ `UploadStats` counters already asserted at zero or exact values in
`tests/zoom_uploads.rs` — but no single place records what the _expected_
numbers are, and no CI or local check compares a new measurement against a
budget and fails. A regression like the one S4a stumbled into should be caught
by a deliberate check, not a retrospective's aside.

## User Story

> "As a `gup-core` implementer finishing a story that touches rendering,
> uploads, pipeline creation or WASM size, I want a recorded performance budget
> and a command that compares my change against it, so that a regression like
> the one between S0b and S4a is caught before it ships instead of discovered by
> chance two stories later."

## Acceptance Criteria

### AC1: The MSAA hypothesis is measured, not assumed

- [ ] `examples/zoom_bench.rs` gains a `--samples 1|4` flag
      (`WindowTarget::set_samples`), so sample count is no longer fixed to the
      default.
- [ ] `zoom_bench` is run on the current tree at `--samples 1` and `--samples 4`
      (otherwise identical: 100K points, `Mailbox --uncapped`, 600 measured
      frames after warm-up, same machine as S0b/S3/S4a), and the GPU pass
      median/p95 and frame interval for both are recorded in a table in this
      story's retrospective, in the same format as the RFC-001 findings tables.
- [ ] `zoom_bench` is also attempted against a checkout of S0b's completion
      commit (around `e6bba47`, before S3 added MSAA) in a scratch worktree. If
      it still builds and runs, its GPU pass number is added to the same table
      as a same-machine, pre-MSAA data point. If it does not build (API drift),
      that is recorded honestly as "not reproducible" rather than skipped
      silently.
- [ ] The table's three-way comparison (pre-MSAA S0b commit if reproducible,
      current tree at 1×, current tree at 4×) states explicitly how much of the
      S0b→S4a GPU-pass gap MSAA accounts for: all, part, or none.
- [ ] **Design decision surfaced, not made silently**: the retrospective states
      S3's visual finding (MSAA helps rules/rects, not analytic circles)
      alongside this story's fps/ms cost finding, and gives an explicit
      recommendation on whether `DEFAULT_SAMPLES` should stay 4. If the cost is
      negligible against the ≥60 fps budget (AC2), the recommendation may be
      "keep 4×, decided here" and the story may land that as a fact in the
      budget doc. If the cost materially eats into headroom, the recommendation
      is "needs an owner decision" (mirroring GUP-406's "Decision needed"
      pattern) and `DEFAULT_SAMPLES` is **not** changed by this story without
      one.

### AC2: Recorded performance budgets for gup-core

- [ ] A checked-in file, `crates/gup-core/PERF_BUDGETS.md`, records, for each of
      the five categories below, the current measured value, the budget (a
      pass/fail threshold with a stated tolerance), the exact command to
      reproduce it, and the machine/toolchain it was measured on (model, driver,
      OS/display server, rustc, date, story).
- [ ] Category: GPU render-pass time (`zoom_bench`, median and p95, ms).
- [ ] Category: CPU-per-frame time (`zoom_bench`, median and p95, ms).
- [ ] Category: uploads per frame during a steady zoom (bytes and write count,
      by `Upload` kind — column must stay 0; uniform and instance bytes have a
      recorded expected value).
- [ ] Category: pipeline create time (`pipeline_timings`, compose+create median
      and cold-run max, ms).
- [ ] Category: WASM size (`mask wasm-size`, gzipped KB, both absolute and the
      over-bare-wgpu delta against RFC-001 §12 risk 10's ≤ +400 KB gz ceiling).
- [ ] Every number in the file is a number this story (or a cited prior
      story/RFC-001 finding) actually measured on the stated machine — no
      invented targets.
- [ ] The file states plainly that GPU/CPU timing budgets are meaningful only on
      real hardware with a real GPU driver (not CI's lavapipe), and names
      `mask perf-budget` (AC3) as the local command that checks them.

### AC3: A regression check that runs where timing is meaningful

- [ ] A new `mask perf-budget` task runs `gup-core`'s wall-clock measurement
      tools (`zoom_bench`, `pipeline_timings`, `mask wasm-size`) and compares
      each result against `PERF_BUDGETS.md`'s recorded value within a stated
      tolerance (e.g. ±20% for GPU/CPU timings, which are driver- and
      thermal-sensitive; an exact or tight-tolerance check for byte/write counts
      and WASM size, which are deterministic). It prints a pass/fail table and
      exits non-zero on any failure.
- [ ] `mask perf-budget` documents that it needs a display/GPU and is a local,
      pre-merge check, not a CI job — matching how `mask gup-core-window` and
      `mask wasm-browser` already document their hardware needs.
- [ ] In CI, where lavapipe makes absolute GPU/CPU timings too slow and noisy to
      budget, the **deterministic proxies already partially in place** are made
      complete and explicit: the existing zero/exact upload-byte assertions
      (`tests/zoom_uploads.rs`), the existing chunk/draw-count assertions
      (`batch.chunks()` in the same test), and a new or extended assertion on
      pipeline count (`Context::pipeline_stats().pipelines_created` does not
      grow across repeated `prepare` calls with an unchanged encoding) are
      documented in `PERF_BUDGETS.md` as the CI-side proxy for "no extra draw,
      no extra upload, no extra pipeline", each naming the test that enforces
      it.
- [ ] `mask wasm-size`'s gzipped-size measurement gains an enforced threshold
      (the ≤ +400 KB gz over-bare-wgpu budget from RFC-001 §12 risk 10) that
      fails the command, not just a report, and that check runs in CI.

### AC4: Seeded violations prove the deterministic checks actually catch regressions

- [ ] Following [GUP-398](GUP-398_Honest_And_Fast_Quality_Gates.md)'s
      seeded-violation convention (check → seeded violation → observed failure,
      recorded in a table), at least two seeded violations are run and recorded.
- [ ] Seed 1: an extra per-frame upload (e.g. an unconditional `write_buffer`
      call added to the zoom path's resolve/prepare step) must fail
      `tests/zoom_uploads.rs`'s byte/write-count assertions.
- [ ] Seed 2: an extra draw call (e.g. an artificially duplicated `ChunkDraw`
      entry) must fail the chunk/draw-count assertion from AC3.
- [ ] Each seed is reverted in the same commit sequence as GUP-398's convention
      (seed, observe, record, revert), and the observed failure (exact assertion
      and message) is recorded in this story's retrospective.

## Technical Tasks

- [ ] Add `--samples 1|4` to `examples/zoom_bench.rs`.
- [ ] Run the three-way MSAA comparison (AC1) and record it.
- [ ] Attempt the S0b-commit scratch-worktree comparison (AC1) and record the
      outcome either way.
- [ ] Write `crates/gup-core/PERF_BUDGETS.md` with the five budget categories
      (AC2), each with a reproduce command, a tolerance, and the machine it was
      measured on.
- [ ] Add the `mask perf-budget` task (AC3), comparing `zoom_bench`,
      `pipeline_timings` and `mask wasm-size` output against `PERF_BUDGETS.md`.
- [ ] Add or extend a pipeline-count assertion (`PipelineStats`) as a
      deterministic CI proxy (AC3).
- [ ] Add an enforced ≤ +400 KB gz threshold to `mask wasm-size` (or a thin
      wrapper/test around it) and wire it into CI (AC3).
- [ ] Run the two seeded violations (AC4), record the observed failures, and
      revert the seeds.
- [ ] Update `.github/agents/story-worker.md`'s Phase 3 Final Validation to add
      a step: for any story that touches `gup-core` rendering, uploads, pipeline
      creation or WASM size, run `mask perf-budget` and record the result in the
      Definition-of-Done evidence, alongside the existing visual-verification
      step.
- [ ] Append a dated "GUP-417 findings" subsection to RFC-001 with the MSAA
      measurement table and the decision (AC1), and a pointer to
      `PERF_BUDGETS.md`.

## Dependencies

### Prerequisite Stories

- GUP-396: RFC-001 S0b — gup-core Vertical Slice, Window Performance ✅ —
  established the `zoom_bench` benchmark, the frame-interval/GPU-pass
  measurement method, and the original 2.7–3.0 ms baseline this story compares
  against.
- GUP-401: RFC-001 S3 — Scene, Renderer, RenderTarget ✅ — made 4× MSAA the
  default (`DEFAULT_SAMPLES`) without measuring its `zoom_bench` cost; this
  story closes that gap.
- GUP-414: RFC-001 S4a — Column Store Chunking, Append ✅ — found the ~3.7 ms
  regression as a side effect and left its cause uninvestigated; this story
  picks that up directly.
- GUP-398: Honest and Fast Quality Gates ✅ — provides the seeded-violation
  convention this story's AC4 follows.

### Enables Stories

- Every subsequent `gup-core` story (S4b/GUP-415 and later) — the DoD addition
  in `.github/agents/story-worker.md` applies `mask perf-budget` to them, and
  `PERF_BUDGETS.md` gives them a baseline to compare against instead of
  discovering drift by chance.

## Testing Strategy

- **Unit tests**: the pipeline-count assertion (AC3) is a `gup-core` unit or
  integration test, run in CI.
- **Integration tests**: `tests/zoom_uploads.rs`'s existing byte/write/draw-
  count assertions are the CI-side deterministic proxy; AC4's seeded violations
  prove they catch regressions.
- **Performance**: `zoom_bench` (GPU pass, CPU, MSAA comparison),
  `pipeline_timings` (pipeline create time), and `mask wasm-size` (WASM size)
  are the three wall-clock tools this story wires into `mask perf-budget` and
  documents in `PERF_BUDGETS.md`. All are run on real hardware, not CI.
- **Visual validation**: not applicable — this story adds measurement and gating
  infrastructure, not rendered output.

## Success Metrics

- [ ] The MSAA hypothesis has a measured answer (all/part/none of the
      regression), recorded with numbers, not asserted.
- [ ] `crates/gup-core/PERF_BUDGETS.md` exists with five recorded, reproducible
      budgets, each tied to a measurement command and a machine.
- [ ] `mask perf-budget` exits non-zero when a measured number falls outside its
      budget's tolerance (proven by a seeded violation or a direct run with a
      deliberately lowered budget).
- [ ] At least two seeded violations (extra upload, extra draw) are shown to
      fail an existing or new deterministic CI check.
- [ ] `mask wasm-size`'s ≤ +400 KB gz budget is enforced in CI, not just
      reported.
- [ ] `.github/agents/story-worker.md`'s Definition of Done names
      `mask perf-budget` for `gup-core` stories.

## Risk Assessment

- **Medium**: GPU/CPU timing numbers are sensitive to thermal state, other load
  on the machine, and driver version, so `mask perf-budget`'s tolerance must be
  generous enough to avoid false failures on a slightly warmer machine, while
  still catching a real ~25% regression like S0b→S4a's. _Mitigation_: a wide
  (~20%) tolerance for GPU/CPU timing, tight tolerances only for the
  deterministic byte/write/draw counts; record in `PERF_BUDGETS.md` that a
  `mask perf-budget` failure on timing alone warrants a second run before
  treating it as real.
- **Medium**: the S0b commit may not build or run cleanly against the current
  toolchain/dependency versions (rustc pin, wgpu, winit have all moved since
  GUP-396). _Mitigation_: AC1 explicitly allows recording "not reproducible" as
  an honest outcome rather than blocking the story on reviving an old commit.
- **Low**: MSAA may turn out to explain only part of the gap, leaving an
  unexplained remainder (GUP-414's other candidates: GUP-407's text/atlas
  changes, or machine state drift between sessions). _Mitigation_: AC1 only
  requires an honest attribution ("all/part/none"), not a complete explanation
  of every millisecond; a remainder, if found, is a finding for a follow-up
  story, not a blocker for this one.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] `mask perf-budget` runs successfully against the recorded budgets on the
      development machine
- [ ] The two seeded violations (AC4) are recorded with their observed failures
      and reverted
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
