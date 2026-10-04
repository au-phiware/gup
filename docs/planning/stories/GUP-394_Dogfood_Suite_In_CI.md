# GUP-394: Dogfood Suite in Repo and CI

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: 📋 Planned **Created**:
2026-10-04

## Context

This story implements **T0.3** of the
[October 2026 strategic review](../STRATEGIC_REVIEW_2026-10.md#t0--guardrails-start-now-never-finish):
promoting the real-world-usage audit's external dogfood crate into the
repository as `dogfood/` and running it in CI. The review's dogfood audit
"completed 3 of 6 realistic tasks, and only with ~50% workaround code" — this
story does not fix any of the gaps the audit found (that is the job of T5 and
the individual follow-up stories the audit's findings motivate); it makes the
audit **repeatable and continuously tracked** instead of a one-off exercise
whose evidence (`/tmp/gup-dogfood-src/`, `/tmp/gup-dogfood/`) is ephemeral and
already partly gone.

The audit's source crate exists at `/tmp/gup-dogfood-src/` (verified present on
2026-10-04; the appendix notes both `/tmp/gup-dogfood-src/` and
`/tmp/gup-dogfood/` — the rendered outputs — are ephemeral, so this story must
copy the **source** crate into the repo before that path is cleaned up by the
environment). Read directly:

- `Cargo.toml`: a detached crate (`[workspace]` with no members, so it does not
  join the parent's workspace automatically), depending on
  `gup = { path = ".." }` as a normal path dependency — i.e. it consumes `gup`
  exactly as an external user would, with no access to `pub(crate)` internals.
  It also depends on `eframe`/`egui` (for the windowed tasks), `tokio`, `csv`,
  `chrono`, `image`, `pollster`, and `bytemuck` (the last with a comment noting
  it's required only because `#[wgsl_function]` expands to bare `bytemuck::`
  paths — GUP-377).
- `run_all.sh`: builds every task binary in release mode against the parent
  `gup` checkout and runs each one, capturing output to `/tmp/gup-dogfood/`. Two
  runs are **expected to fail/show specific symptoms**: `t6_wgsl` run with
  `RAW_MACRO=1` is expected to panic (duplicate uniforms struct), and
  `t5_stream` is expected to produce blank PNGs ("stream never render-ready").
- The six tasks, read from `src/bin/*.rs` doc comments (`gen_data.rs` and
  `ref_export_png.rs` are fixtures/reference code, not tasks; `t0_smoke.rs` is a
  control case):
  1. **`t1_timeseries`** — multi-line time-series from CSV with a date axis,
     legend, title, PNG export. Its doc comment documents workarounds needed
     today: domains computed by hand, a time-unit guess for the date formatter,
     and hand-built SVG line paths + legend (rasterised via ImageMagick) because
     PNG export has no text and SVG export has no data marks.
  2. **`t2_bars`** — grouped + stacked bar chart with legend and value labels.
     Its doc comment documents that `group_by`/`stack_by` don't affect bar
     geometry, `.color(String)` renders grey, and there is no legend/
     value-label API, so the "workaround" variant fakes all three.
  3. **`t3_scatter_png`** / **`t3_scatter_window`** — a 200k-point scatter,
     colour = categorical segment, size = numeric weight, log-scale X, as a PNG
     and (separately) in a window with a hand-built hover tooltip (its doc
     comment notes `GupApp` has no input hooks, hover-reveal only reveals
     clipped label text, and `gup-egui` doesn't compile, so the task uses raw
     `eframe` + a hand-written hit test).
  4. **`t4_linked`** — two linked scatter views in egui using
     `SharedSelectionState` for brush-linked highlighting.
  5. **`t5_live`** / **`t5_stream`** — live-appending data: `t5_live` appends
     points every 100ms via `set_data` + `prepare_render_bound` (its doc comment
     notes there is no append API, so every tick re-sends all data); `t5_stream`
     is the alternate `DataStream`/`Selection::stream()` route, expected to
     produce blank PNGs.
  6. **`t6_wgsl`** — a custom `#[wgsl_function]` GPU transform in a downstream
     crate, following Tutorial 3, with a documented `RAW_MACRO=1` failure mode
     and a `use gup::*;` workaround comment referencing GUP-377.

## User Story

> "As a maintainer, I want the dogfood audit's six realistic tasks checked into
> the repo and run in CI with pixel assertions, so that 'does a real user get a
> working chart' is answered on every change instead of once, in a throwaway
> `/tmp` crate, by a one-time audit."

## Acceptance Criteria

### AC1: Dogfood crate promoted into the repository

- [ ] `/tmp/gup-dogfood-src/` is copied into `dogfood/` at the repository root,
      preserving its structure (`Cargo.toml`, `run_all.sh`, `src/bin/*.rs`,
      `src/lib.rs`, `src/main.rs`).
- [ ] `dogfood/Cargo.toml`'s `gup = { path = ".." }` dependency is updated to
      the correct relative path from the new location (`path = ".."` remains
      correct if `dogfood/` sits directly under the repo root next to the main
      crate — verify).
- [ ] `dogfood/` remains a **detached** crate (its own `[workspace]`, not a
      member of the parent workspace), preserving the property that it consumes
      `gup` as a true external dependency with no `pub(crate)` access — this is
      the whole point of the exercise per its own `Cargo.toml` comment.
- [ ] `dogfood/` is added to `.gitignore` exclusions as needed (its own
      `target/` directory) and a `README.md` explaining what it is, why it's
      detached, and how to run it (`./run_all.sh`) is added.

### AC2: CI runs the dogfood suite with pixel assertions

- [ ] A CI job builds `dogfood/` against the current `gup` checkout and runs
      each task binary.
- [ ] Tasks that produce PNG output (`t0_smoke`, `t1_timeseries`, `t2_bars`,
      `t3_scatter_png`, `t5_stream`, `ref_export_png`) have pixel-level
      assertions in CI (non-blank, text-present where configured, expected
      colours present) — reuse structural-assertion helpers from GUP-388's
      visual regression harness if that story has landed, rather than
      duplicating assertion logic.
- [ ] Windowed tasks (`t3_scatter_window`, `t4_linked`, `t5_live`) run with
      `DOGFOOD_AUTO=1` (the existing scripted-input-and-screenshot mode, per
      `run_all.sh`) and their resulting screenshots get the same pixel
      assertions.

### AC3: Expected failures are tracked, not hidden

- [ ] The two known-failing/known-degenerate cases from `run_all.sh` — `t6_wgsl`
      with `RAW_MACRO=1` (expected panic) and `t5_stream` (expected blank PNGs)
      — are represented in CI as **tracked expected failures** (e.g. the job
      asserts the panic/blank-output actually still occurs, so a silent fix
      isn't lost, and fails loudly if the expected-failure resolves, prompting
      someone to update the tracking).
- [ ] CI output clearly distinguishes "task succeeded," "task is a tracked
      expected failure," and "task failed unexpectedly" — the last is the only
      case that should fail the build.

### AC4: Accurate task documentation

- [ ] This story's completion evidence (or a `dogfood/README.md`) accurately
      describes each of the six tasks and their current known gaps, matching
      what was found reading the actual source (per the Context section above)
      rather than re-stating the strategic review's summary prose.

## Technical Tasks

- [ ] Copy `/tmp/gup-dogfood-src/` into `dogfood/` before the ephemeral
      environment cleans it up — do this first, before any other work in this
      story.
- [ ] Verify `dogfood/Cargo.toml`'s path dependency and run
      `cd dogfood && cargo build --release --bins` to confirm it still builds
      against the current parent checkout (expect some build failures if
      GUP-389/390/391 have landed and changed the public API — fix the dogfood
      crate's usage to match, since that is itself exactly the kind of
      external-crate signal this suite exists to surface; do not weaken the
      suite to avoid the friction).
- [ ] Add a CI workflow (or extend an existing one) that runs
      `dogfood/run_all.sh` or an equivalent CI-friendly invocation.
- [ ] Add pixel-assertion tooling for the PNG-producing tasks, reusing GUP-388's
      structural-assertion helpers where available.
- [ ] Encode the two expected-failure cases explicitly in the CI job logic.
- [ ] Write `dogfood/README.md`.
- [ ] Add `dogfood/target/` to `.gitignore`.

## Dependencies

### Prerequisite Stories

- GUP-377: Fix `#[wgsl_function]` Crate Path Resolution ✅ — the `t6_wgsl` task
  depends on this fix already being in place (confirmed by its own `use gup::*;`
  workaround comment referencing GUP-377, which may now be removable — check at
  implementation time whether the workaround is still needed).

### Enables Stories

- None directly tracked yet; this suite is expected to surface new follow-up
  stories once it runs continuously (e.g. gaps found in `t2_bars`'s
  `group_by`/`stack_by`/legend handling), but per this project's guidance, those
  should be filed as their own stories once observed in CI, not pre-emptively
  listed here.

## Testing Strategy

- **Build verification**: `cd dogfood && cargo build --release --bins` succeeds
  against the current parent checkout.
- **CI run**: the dogfood CI job runs end-to-end at least once successfully
  (modulo the two tracked expected failures) before this story is marked done.
- **Pixel assertions**: each PNG-producing task's output is checked for
  non-blank, text-presence (where configured), and expected-colour presence, per
  AC2.

## Success Metrics

- [ ] `dogfood/` exists in the repository and builds against the current `gup`
      checkout.
- [ ] CI runs all six tasks (plus `t0_smoke`/`gen_data`/`ref_export_png`) on
      every relevant change, with pixel assertions on PNG output.
- [ ] The two expected-failure cases are explicitly tracked, and CI fails loudly
      if either one silently starts passing (prompting the tracking to be
      updated) or if the count of unexpectedly-failing tasks increases.

## Risk Assessment

- **Medium**: The dogfood crate currently depends on `eframe`/`egui` for its
  windowed tasks, independent of the parked `gup-egui` integration crate (it
  uses `eframe` directly, not `gup-egui`) — confirm this dependency still builds
  in CI's headless environment (software rendering or Xvfb may be needed for
  `eframe`'s windowed tasks even in `DOGFOOD_AUTO=1` mode). Mitigation: verify
  the existing CI environment's GPU/display setup (used for the crate's own
  windowed example tests) covers this, or extend it.
- **Medium**: If GUP-389/390/391 land first and change the public API the
  dogfood crate depends on, this story's build step will surface real breakage.
  Mitigation: this is intended behaviour (per AC's instruction not to weaken the
  suite), but budget time in this story for fixing the dogfood crate's call
  sites to match the new API, not just for the CI wiring itself.
- **Low**: `/tmp/gup-dogfood-src/` is ephemeral and could already be gone by the
  time this story is picked up. Mitigation: if gone, the six tasks must be
  reconstructed from the strategic review's appendix description and the
  original audit's retrospective — note this explicitly in the retrospective if
  it happens, since the reconstruction will be less precise than a direct copy.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1` (dogfood crate is
      separate from the main workspace; also run
      `cargo build --release     --bins` inside `dogfood/`).
- [ ] Lint and format clean: `mask all-fix`.
- [ ] All examples compile: `cargo check --examples`.
- [ ] Story status updated to ✅ Complete in story file and INDEX.md.
- [ ] Retrospective added to story document, including which (if any) dogfood
      tasks needed call-site updates due to API changes from other wave-1
      stories landing first.
