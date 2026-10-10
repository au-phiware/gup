# GUP-417: Track gup-core Performance Budgets

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-10)
**Created**: 2026-10-10

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

- [x] `examples/zoom_bench.rs` gains a `--samples 1|4` flag
      (`WindowTarget::set_samples`), so sample count is no longer fixed to the
      default.
- [x] `zoom_bench` is run on the current tree at `--samples 1` and `--samples 4`
      (otherwise identical: 100K points, `Mailbox --uncapped`, 600 measured
      frames after warm-up, same machine as S0b/S3/S4a), and the GPU pass
      median/p95 and frame interval for both are recorded in a table in this
      story's retrospective, in the same format as the RFC-001 findings tables.
- [x] `zoom_bench` is also attempted against a checkout of S0b's completion
      commit (around `e6bba47`, before S3 added MSAA) in a scratch worktree. If
      it still builds and runs, its GPU pass number is added to the same table
      as a same-machine, pre-MSAA data point. If it does not build (API drift),
      that is recorded honestly as "not reproducible" rather than skipped
      silently.
- [x] The table's three-way comparison (pre-MSAA S0b commit if reproducible,
      current tree at 1×, current tree at 4×) states explicitly how much of the
      S0b→S4a GPU-pass gap MSAA accounts for: all, part, or none.
- [x] **Design decision surfaced, not made silently**: the retrospective states
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

- [x] A checked-in file, `crates/gup-core/PERF_BUDGETS.md`, records, for each of
      the five categories below, the current measured value, the budget (a
      pass/fail threshold with a stated tolerance), the exact command to
      reproduce it, and the machine/toolchain it was measured on (model, driver,
      OS/display server, rustc, date, story).
- [x] Category: GPU render-pass time (`zoom_bench`, median and p95, ms).
- [x] Category: CPU-per-frame time (`zoom_bench`, median and p95, ms).
- [x] Category: uploads per frame during a steady zoom (bytes and write count,
      by `Upload` kind — column must stay 0; uniform and instance bytes have a
      recorded expected value).
- [x] Category: pipeline create time (`pipeline_timings`, compose+create median
      and cold-run max, ms).
- [x] Category: WASM size (`mask wasm-size`, gzipped KB, both absolute and the
      over-bare-wgpu delta against RFC-001 §12 risk 10's ≤ +400 KB gz ceiling).
- [x] Every number in the file is a number this story (or a cited prior
      story/RFC-001 finding) actually measured on the stated machine — no
      invented targets.
- [x] The file states plainly that GPU/CPU timing budgets are meaningful only on
      real hardware with a real GPU driver (not CI's lavapipe), and names
      `mask perf-budget` (AC3) as the local command that checks them.

### AC3: A regression check that runs where timing is meaningful

- [x] A new `mask perf-budget` task runs `gup-core`'s wall-clock measurement
      tools (`zoom_bench`, `pipeline_timings`, `mask wasm-size`) and compares
      each result against `PERF_BUDGETS.md`'s recorded value within a stated
      tolerance (e.g. ±20% for GPU/CPU timings, which are driver- and
      thermal-sensitive; an exact or tight-tolerance check for byte/write counts
      and WASM size, which are deterministic). It prints a pass/fail table and
      exits non-zero on any failure.
- [x] `mask perf-budget` documents that it needs a display/GPU and is a local,
      pre-merge check, not a CI job — matching how `mask gup-core-window` and
      `mask wasm-browser` already document their hardware needs.
- [x] In CI, where lavapipe makes absolute GPU/CPU timings too slow and noisy to
      budget, the **deterministic proxies already partially in place** are made
      complete and explicit: the existing zero/exact upload-byte assertions
      (`tests/zoom_uploads.rs`), the existing chunk/draw-count assertions
      (`batch.chunks()` in the same test), and a new or extended assertion on
      pipeline count (`Context::pipeline_stats().pipelines_created` does not
      grow across repeated `prepare` calls with an unchanged encoding) are
      documented in `PERF_BUDGETS.md` as the CI-side proxy for "no extra draw,
      no extra upload, no extra pipeline", each naming the test that enforces
      it.
- [x] `mask wasm-size`'s gzipped-size measurement gains an enforced threshold
      (the ≤ +400 KB gz over-bare-wgpu budget from RFC-001 §12 risk 10) that
      fails the command, not just a report, and that check runs in CI.

### AC4: Seeded violations prove the deterministic checks actually catch regressions

- [x] Following [GUP-398](GUP-398_Honest_And_Fast_Quality_Gates.md)'s
      seeded-violation convention (check → seeded violation → observed failure,
      recorded in a table), at least two seeded violations are run and recorded.
- [x] Seed 1: an extra per-frame upload (e.g. an unconditional `write_buffer`
      call added to the zoom path's resolve/prepare step) must fail
      `tests/zoom_uploads.rs`'s byte/write-count assertions.
- [x] Seed 2: an extra draw call (e.g. an artificially duplicated `ChunkDraw`
      entry) must fail the chunk/draw-count assertion from AC3.
- [x] Each seed is reverted in the same commit sequence as GUP-398's convention
      (seed, observe, record, revert), and the observed failure (exact assertion
      and message) is recorded in this story's retrospective.

## Technical Tasks

- [x] Add `--samples 1|4` to `examples/zoom_bench.rs`.
- [x] Run the three-way MSAA comparison (AC1) and record it.
- [x] Attempt the S0b-commit scratch-worktree comparison (AC1) and record the
      outcome either way.
- [x] Write `crates/gup-core/PERF_BUDGETS.md` with the five budget categories
      (AC2), each with a reproduce command, a tolerance, and the machine it was
      measured on.
- [x] Add the `mask perf-budget` task (AC3), comparing `zoom_bench`,
      `pipeline_timings` and `mask wasm-size` output against `PERF_BUDGETS.md`.
- [x] Add or extend a pipeline-count assertion (`PipelineStats`) as a
      deterministic CI proxy (AC3).
- [x] Add an enforced ≤ +400 KB gz threshold to `mask wasm-size` (or a thin
      wrapper/test around it) and wire it into CI (AC3).
- [x] Run the two seeded violations (AC4), record the observed failures, and
      revert the seeds.
- [x] Update `.github/agents/story-worker.md`'s Phase 3 Final Validation to add
      a step: for any story that touches `gup-core` rendering, uploads, pipeline
      creation or WASM size, run `mask perf-budget` and record the result in the
      Definition-of-Done evidence, alongside the existing visual-verification
      step.
- [x] Append a dated "GUP-417 findings" subsection to RFC-001 with the MSAA
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

- [x] The MSAA hypothesis has a measured answer (all/part/none of the
      regression), recorded with numbers, not asserted.
- [x] `crates/gup-core/PERF_BUDGETS.md` exists with five recorded, reproducible
      budgets, each tied to a measurement command and a machine.
- [x] `mask perf-budget` exits non-zero when a measured number falls outside its
      budget's tolerance (proven by a seeded violation or a direct run with a
      deliberately lowered budget).
- [x] At least two seeded violations (extra upload, extra draw) are shown to
      fail an existing or new deterministic CI check.
- [x] `mask wasm-size`'s ≤ +400 KB gz budget is enforced in CI, not just
      reported.
- [x] `.github/agents/story-worker.md`'s Definition of Done names
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

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1` (every
      gup-core test; `zoom_uploads` also on lavapipe as CI runs it). The root
      crate's tests were not built (disk; this story changes no root-crate code)
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check -p gup-core --examples` (the only
      examples this story touches)
- [x] `mask perf-budget` runs successfully against the recorded budgets on the
      development machine
- [x] The two seeded violations (AC4) are recorded with their observed failures
      and reverted
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- **`zoom_bench --samples 1|4`** (`WindowTarget::set_samples`); the run line
  names the sample count, and the report ends with `metric <key> <value>` lines
  (GPU pass and CPU median/p95, uploads per frame by kind, window size).
  `pipeline_timings` prints `metric` lines too (link+create median, cold run,
  max).
- **`crates/gup-core/PERF_BUDGETS.md`**: the machine, a machine-readable budget
  table (22 metrics, each `exact`, `+N%`, `<= X` or `info`), the five categories
  with their commands, recorded values and tolerances, the CI-side proxies and
  the tests that enforce them, and the history.
- **`mask perf-budget`** (`scripts/perf_budget.sh`): runs `zoom_bench`
  (`Mailbox` uncapped), `pipeline_timings` and `scripts/wasm_size.sh`, samples
  the i915 GPU clock during `zoom_bench`, prints a pass/fail table and exits
  non-zero on any failure. `--budgets`, `--metrics`, `--skip-wasm`.
- **`mask wasm-size`** moved into `scripts/wasm_size.sh`, which fails above
  400,000 B gz over bare wgpu and checks both harnesses pin the installed
  `wasm-bindgen`. The Visual regression workflow's browser job runs it (its
  cache key now includes the baseline harness's lockfile), as does
  `mask ci visual-regression`.
- **CI proxies** in `tests/zoom_uploads.rs`: frames now go through
  `Renderer::prepare` + one pass + `present` (the window loop's path), and the
  test pins exact uniform bytes and writes per frame, one instance write per
  guide draw, a guide-byte ceiling, a constant draw-call count
  (`Prepared::draw_calls`, new public method) and no program or pipeline after
  the first frame.
- **Docs**: RFC-001 "GUP-417 findings"; `.github/agents/story-worker.md` Phase 3
  step 9 (run `mask perf-budget` for gup-core stories);
  `.github/workflows/README.md`.

Key files: `crates/gup-core/examples/zoom_bench.rs`,
`crates/gup-core/src/render.rs`, `crates/gup-core/src/selection.rs`,
`crates/gup-core/tests/zoom_uploads.rs`, `crates/gup-core/PERF_BUDGETS.md`,
`scripts/perf_budget.sh`, `scripts/wasm_size.sh`, `maskfile.md`,
`.github/workflows/visual-regression.yml`.

Tests: gup-core's 84 lib tests and every integration test pass; `zoom_uploads`
keeps its 2 tests, now with 7 more assertions each. `mask all-check` passes.

### Definition-of-Done evidence: `mask perf-budget`

```text
metric                                 recorded    check        limit     measured  verdict
zoom.size                             1920x1080    exact    1920x1080    1920x1080  pass
zoom.samples                                  4    exact            4            4  pass
zoom.gpu_clock.median_mhz                   350     info            -          467  info
zoom.gpu_pass.median_ms                    3.89     +20%        4.668        3.721  pass
zoom.gpu_pass.p95_ms                       6.30     +25%        7.875        6.064  pass
zoom.cpu.median_ms                         0.76     +20%        0.912        0.780  pass
zoom.cpu.p95_ms                            1.13     +25%        1.412        0.998  pass
zoom.interval.median_ms                    4.91     info            -        4.508  info
zoom.columns.bytes                            0    exact            0            0  pass
zoom.columns.writes                           0    exact            0            0  pass
zoom.validity.bytes                           0    exact            0            0  pass
zoom.uniforms.bytes_per_frame                88    exact           88           88  pass
zoom.uniforms.writes_per_frame                3    exact            3            3  pass
zoom.instances.bytes                    5629476    exact      5629476      5629476  pass
zoom.instances.writes_per_frame               3    exact            3            3  pass
zoom.textures.bytes                           0    exact            0            0  pass
pipeline.link_create.median_ms             0.88     +20%        1.056        0.891  pass
pipeline.link_create.cold_ms               1.04    <= 10           10        1.100  pass
pipeline.link_create.max_ms                1.06     info            -        1.179  info
wasm.wgpu.gz_bytes                        41712     info            -        41712  info
wasm.scatter.gz_bytes                    252925      +2%   257983.500       252925  pass
wasm.over_wgpu.gz_bytes                  211213 <= 400000       400000       211213  pass

perf-budget: every check passed
```

## Retrospective

**Completed**: 2026-10-10

### The MSAA measurement (AC1)

Method: three release binaries run alternately, on the S0b/S3/S4a machine (Intel
HD Graphics 630, Mesa 26.0.0 Vulkan, niri at 1920×1080 60.02 Hz, rustc 1.93.1):
S0b's completion commit `e6bba47` (built in a scratch worktree under
`~/.cache/gup/scratch` with its own target directory, since it predates
GUP-411's `.cargo/config.toml`; it built and ran unchanged), and this tree at
`--samples 1` and `--samples 4`. `cmp` and `sha256sum` confirmed the S0b and
current binaries differ. 100K points, fullscreen, 60 warm-up + 600 measured
frames; each run sampled `/sys/class/drm/card1/gt_act_freq_mhz` every 20 ms.
Cells are the median of the runs' medians (p95s); the scratch worktree and its
target directory were deleted afterwards.

| `Mailbox` uncapped, 6 runs each    | Frame interval median / p95 (ms) | fps median / p95 | CPU work median / p95 (ms) | GPU pass median / p95 (ms) | GPU pass median, range |
| ---------------------------------- | -------------------------------: | ---------------: | -------------------------: | -------------------------: | ---------------------: |
| S0b (`e6bba47`), 1 sample          |                    4.455 / 6.833 |    224.5 / 146.3 |              0.688 / 0.856 |              2.984 / 5.860 |            2.76 – 3.01 |
| this tree, `--samples 1`           |                    4.526 / 6.910 |    220.9 / 144.7 |              0.686 / 0.864 |              3.024 / 5.868 |            2.94 – 3.04 |
| this tree, `--samples 4` (default) |                    4.908 / 7.326 |    203.7 / 136.5 |              0.762 / 1.127 |              3.888 / 6.297 |            3.73 – 4.14 |

| `Fifo` (vsync), 3 runs each        | fps median / p95 | CPU work median (ms) | GPU pass median / p95 (ms) |
| ---------------------------------- | ---------------: | -------------------: | -------------------------: |
| S0b (`e6bba47`), 1 sample          |      60.0 / 59.4 |                0.771 |              3.033 / 5.711 |
| this tree, `--samples 1`           |      60.0 / 59.4 |                0.769 |              3.008 / 5.537 |
| this tree, `--samples 4` (default) |      60.0 / 59.4 |                0.778 |              4.703 / 8.296 |

**Attribution: MSAA accounts for all of the S0b→S4a GPU-pass gap.** The pre-MSAA
S0b tree and this tree at 1 sample agree within 1.5%; 4 samples add +0.86 ms
median uncapped (+29%) and +1.7 ms under vsync at the clock floor (+56%), which
is the ~3.7 ms S4a saw. Nothing else between S0b and S4b costs anything
measurable on the GPU.

**Design decision surfaced: `DEFAULT_SAMPLES` needs an owner decision.** S3's
visual finding: 4× matters only for geometric edges (a fan of 1.5 px diagonal
rules has 0 partially covered pixels at 1× and 597 at 4×; a fractional rect 0
and 71; circles antialias analytically and differ by ≤ 2/255). This story's cost
finding: +29–56% of the GPU pass. Against the ≥ 60 fps budget the default still
holds (vsync: 60.0 / 59.4 fps, no missed refresh, GPU p95 8.3 of 16.7 ms at 4×
against 5.5 at 1×), but the relative cost is not negligible, so
`DEFAULT_SAMPLES` stays 4 and the options go to the owner (RFC-001 "GUP-417
findings"): keep 4× (**recommended**: the target holds with 2× headroom on a
2017 iGPU and the guides look right; revisit at S9's dataset sizes); default to
1× with 4× opt-in (saves 0.9–1.7 ms, guides alias, 4× goldens re-blessed); or
antialias rules and rects analytically like circles and then default to 1× (a
story of its own). The budgets are recorded at the current default, 4×.

### Seeded violations (AC4)

Each seed was made in the working tree,
`cargo test -p gup-core --test zoom_uploads` (or the named command) was run, the
failure recorded, and the seed reverted with `git checkout` before anything was
committed.

| Check                                                     | Seeded violation                                                                                                          | Observed failure                                                                                                                                  |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| uniform writes per frame                                  | seed 1: an unconditional extra `cx.write_buffer(Upload::Uniform, &gpu.view_buffer, 0, &[0u8; 16])` in `Renderer::prepare` | both tests: `assertion left == right failed: UploadStats { … uniforms: WriteCount { bytes: 31200, writes: 1200 } … }`, `left: 1200`, `right: 900` |
| guide-byte ceiling                                        | seed 1b: guide instances padded to the pooled buffer's whole capacity before `write_buffer`                               | both tests: `guide instances: 1544736 B over 300 frames, budget 1387872 B; if the guides changed on purpose, record the new total …`              |
| mark draws = chunks                                       | seed 2: chunk 0's `ChunkDraw` duplicated in `selection.rs` (`.chain(store.chunks().iter().enumerate().take(1))`)          | `assertion left == right failed`, `left: 2`, `right: 1` (1 chunk); `left: 8`, `right: 7` (7 chunks)                                               |
| draw calls per frame                                      | seed 2b: the scene's last item prepared twice (`scene.items.iter().chain(scene.items.last())`)                            | `assertion left == right failed: draw calls in the first frame`, `left: 5`, `right: 4`; `left: 11`, `right: 10`                                   |
| no pipeline after frame 0                                 | seed 3: `Context::mark_pipeline` skips its cache hit                                                                      | `a pipeline was created after the first frame: PipelineStats { programs_linked: 1, pipelines_created: 302, … }`, `left: 302`, `right: 2`          |
| WASM ceiling (`scripts/wasm_size.sh`)                     | `GUP_WASM_OVER_WGPU_MAX_GZ=200000`                                                                                        | exit 1, `wasm-size: FAIL: gup-core costs 211213 B gz over bare wgpu, over the 200000 B budget`                                                    |
| `mask perf-budget` (`scripts/perf_budget.sh --budgets …`) | a copy of `PERF_BUDGETS.md` with the GPU median lowered to 2.00 and instance writes per frame to 2, then a full real run  | exit 1, `zoom.gpu_pass.median_ms … 2.400 4.090 FAIL`, `zoom.instances.writes_per_frame … 2 3 FAIL`, `2 check(s) FAILED`                           |

The first attempt at seed 3 removed the cache check inside the error scope and
the test still passed: `mark_pipeline` has an earlier `hit()` check, and the
inner one only guards a race. Seeding the outer check failed the test as above.

### Key Technical Learnings

#### The GPU clock governor is the biggest source of noise

- **Challenge**: the first 4× runs gave GPU pass medians of 4.27, 3.97, 1.87 and
  1.90 ms for one binary: bimodal, a 2.3× spread no tolerance could absorb.
- **Solution**: sampling `gt_act_freq_mhz` during each run showed the i915
  governor either holding the 350 MHz floor or boosting to ~1050 MHz; boosted
  runs are twice as fast. Pinning needs root, so the comparisons were made
  between runs at the same clock state (a quiet machine kept every run at the
  floor), the budgets were recorded at the floor (the slow state, so a boosted
  run can only pass), and `mask perf-budget` samples the clock and says when it
  was boosted. Under `Fifo` the clock never left the floor.
- **Pattern**: record the GPU clock with any GPU timing on an integrated GPU.
  Earlier findings tables (S0b, S4a) did not, which is part of why the gap
  looked mysterious.

#### Pipeline creation time is Mesa's shader cache

- **Challenge**: `pipeline_timings`' cold run varied from 0.94 to 4.47 ms, and
  GUP-406 had recorded 8.4 ms.
- **Solution**: with `MESA_SHADER_CACHE_DISABLE=true` the median is 5.0 ms and
  the cold run 5.3–7.8 ms. The cold run is a single sample, so it got an
  absolute ceiling (10 ms) rather than a relative tolerance; the median carries
  the +20% budget.
- **Pattern**: a single-sample timing gets an absolute ceiling set against a
  known structural regression, not a relative tolerance.

#### Deterministic proxies need the frame's real path

- **Challenge**: `Renderer::render` hides the `Prepared`, so the draw count was
  not observable, and calling `prepare` a second time would have doubled the
  upload counts the test pins.
- **Solution**: the test now prepares, draws and presents each frame itself,
  like `zoom_bench` and the window loop, and `Prepared::draw_calls` counts what
  `draw` records.

### Architectural Decisions

#### Budgets as a machine-readable table in Markdown

- **Decision**: `PERF_BUDGETS.md`'s budget table is the single source the script
  parses (backticked metric, recorded value, check); the tools print
  `metric <key> <value>` lines.
- **Reasoning**: one file for humans and the check, so they cannot drift; the
  `metric` lines decouple the check from the tools' human-readable output.
- **Trade-off**: the table's format is now an interface; a malformed row is
  silently ignored, but a missing metric fails as "not measured".
- **Future**: another machine can keep its own budgets file and pass it with
  `--budgets`.

#### Fail on slower only; flag faster

- **Decision**: `+N%` fails only above the recorded value; a run more than N%
  faster passes with "re-record?".
- **Reasoning**: getting faster is not a regression, but a stale budget weakens
  the check.

#### The CI WASM check lives in the browser job

- **Decision**: `scripts/wasm_size.sh` runs in the Visual regression workflow's
  browser job.
- **Reasoning**: that job already installs the toolchain, the wasm32 target and
  the matching `wasm-bindgen`, and caches the scatter harness's build; only the
  small bare-wgpu harness is new.

### Development Workflow Insights

- A usage-limit interruption landed mid-story; every increment had been
  committed (checkpoint rule), so nothing was lost.
- The machine was loaded (a `java` process at 100% CPU, load average 6–9) during
  the first measurements, which were bimodal; the decisive runs were taken when
  it was quiet (load ~2). Alternating binaries run by run kept drift from
  favouring one configuration.
- The S0b commit built and ran unchanged against today's toolchain: AC1's "not
  reproducible" fallback was not needed. Its own target directory cost 321 MB.
- No Python in the dev shell: file edits went through `perl -0pi` and the Edit
  tool.

### Follow-up Stories

None written. The one candidate, analytic antialiasing for rules and rects so
the default can drop to 1×, depends on the owner's `DEFAULT_SAMPLES` decision
(RFC-001 "GUP-417 findings", option 3) and should be written only if the owner
chooses it. Pinning the GPU clock for benchmarks needs root and is documented
instead.
