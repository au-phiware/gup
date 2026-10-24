# Strategic Review — November 2026 (post-migration)

**Status**: Proposed, awaiting the owner's [decisions](#decisions-for-the-owner)
**Date**: 2026-10-24 **Previous**: [October 2026](./STRATEGIC_REVIEW_2026-10.md)

## Summary

RFC-001 is complete. In 20 days (2026-10-04 to 10-24, 540 commits, about 80
stories) the old core was replaced: `gup` is a 77-line facade over `gup-core`
(28.9k lines), about 209k lines were deleted in the flip, and the old-path
metric was retired at 0. Every finding of the October review that made Gup "not
a product" is fixed. **What is left is breadth, not architecture.** A user can
draw three chart types well, and cannot yet install the crate, read a current
tutorial, draw an area chart or histogram, or give a chart a text alternative.

The process worked, but its binding constraint is now tokens. The weekly limit
stopped all work for 9 of the 20 days (10-11 to 10-16 and 10-19 to 10-23).

## 1. Where we are against the north star

> A user writes ~5–10 lines and gets a publication-quality chart, identical in a
> window, PNG, SVG and WASM, with encodings evaluated on the GPU, and can drop
> into the low-level `Selection` API without starting over.

| October finding                        | Now                                                                                                                                               |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| Shader-function core unused            | Every chart goes through `ShaderFn` scales with f64 CPU mirrors and the GPU column store. Zooming writes 0 column bytes (exact budget).           |
| Output broken by default               | Text, legends, grid and sRGB-correct colours in PNG; SVG writes the full scene (200k circles in dogfood t3); goldens for every builder.           |
| API hard to use                        | One `Context`, a prelude, `hello_world.rs` is 16 lines and the scatter in the north star is a passing doctest; `chart.into_plot()` drops a level. |
| Not adoptable (dogfood 3/6, 50% hacks) | Dogfood 10 PASS / 0 XFAIL / 0 FAIL on the public API; egui and Bevy integrations on `gup-core`.                                                   |
| 40k unwired LOC                        | Old path deleted. Parked: `gup-culling-lod`, mobile, Tauri.                                                                                       |

**Measured scale and interactivity** (`PERF_BUDGETS.md`, HD 630): 4M sorted rows
draw in 9.9 ms of GPU pass with chunk culling (14 draws); 10M rows stream
through `LastRows(100_000)` at 0.60 ms per batch; GPU pick 2.3 ms, CPU pick 12
µs after a 14 ms grid build at 100k; WASM scatter 334,149 B gz (98 KB under the
400 KB cap). Hover, tooltips, wheel zoom, drag pan, brush and drag-zoom work in
a window, egui, Bevy and the browser. The "billion-point" claim in `README.md`
is not demonstrated: rows enter through per-row CPU closures, LOD is parked, and
nothing above 10M rows has been measured.

### What a real user still cannot do

| Area          | State                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Chart types   | Builders: `scatter`, `line`, vertical `bar` (group, stack). Marks: `Circle`, `Rect`, `Segment`, `Line`. **Missing**: area/stacked area, horizontal bars, histogram (no bin transform), heatmap builder, box/violin, density, pie, choropleth, error bars, line curves, a text/annotation mark (so no bar value labels), facets, subtitle.                                                                                 |
| Data input    | `impl IntoIterator<Item = T>` plus closures, which covers `Vec<Row>`. No CSV, serde, Arrow or Polars path; `ColumnStore::from_columns` exists but is not reachable from `Plot` or the builders. Columnar input is the route to GPU scale.                                                                                                                                                                                 |
| Export        | PNG at any dpr, SVG (default-on `svg` feature), window, texture, WebGPU canvas. **PDF was deleted in GUP-486** with `printpdf`. It is a real gap for journal figures, but SVG→PDF is a solved problem: `svg2pdf` over `SvgTarget` keeps one vector path.                                                                                                                                                                  |
| Accessibility | **None.** The old `src/accessibility` (about 264 KB: ARIA, AT-SPI, VoiceOver, UI Automation, keyboard focus, high contrast, patterns, sonification) was deleted in the flip (GUP-488). `gup-core` has no keyboard navigation, no text alternative in SVG or on the canvas, no high-contrast theme. Okabe-Ito is the only a11y-friendly default. Eight guides and `docs/accessibility/` still describe the deleted system. |
| Text          | Inter only, no OpenType shaping (GUP-405: kerning, tabular figures) and no font fallback (GUP-416), so non-Latin labels are not supported.                                                                                                                                                                                                                                                                                |
| Themes        | `Theme::default()` and struct-update overrides; no dark preset (October's T4b asked for light and dark).                                                                                                                                                                                                                                                                                                                  |
| Platforms     | CI is Linux-only (lavapipe, SwiftShader). macOS (Metal) and Windows (DX12) are untested. Mobile and Tauri are parked and do not compile (they name the deleted `ios-shim` and `android-shim` features).                                                                                                                                                                                                                   |
| Docs          | `TECHNICAL_APPROACH`, `IMPLEMENTATION_STRATEGY` and `README` are current and doctest-backed (GUP-489). Six tutorials, the feature guides and the `gup-core` crate docs describe deleted APIs (see triage).                                                                                                                                                                                                                |
| crates.io     | Not publishable. `gup-core`, `gup-core-macros`, `gup-wgsl` and `gup-text` are `publish = false` with path-only dependencies; `README.md` already tells users `gup = "0.1"`. Licence is GPL-3.0-or-later, unusual for a Rust library and a barrier to adoption (Decision 1). The crate name is unchecked.                                                                                                                  |

**Verdict.** The north-star sentence is true for scatter, line and bar, and the
architecture under it is sound. A newcomer who finds Gup today hits a stale
tutorial, a crate that is not on crates.io and three chart types. The next month
should make Gup _usable by someone else_, not extend the core.

## 2. Process retrospective (2026-10-04 to 10-24)

### What worked

- **Decision-complete stories plus model tiering.** Opus scribes wrote stories
  with Read First, Decisions, Escalation Triggers and Out of Scope, at 300–396K
  tokens per batch of 7–16 stories (about 25–45K a story). Sonnet workers
  finished them cleanly at 60–171K (median about 105K), against 480–505K per
  story on Opus before tiering: roughly 4× cheaper with no loss in accepted
  quality. Opus implementers (GUP-431, 432, 455, 482) cost 205–271K.
- **Escalations caught real bugs before merge.** GUP-444: zoom made the domain
  explicit and flipped the range policy. GUP-479/494: end bars clipped and
  baselines missing from domain fits. GUP-483: egui double-decoded an sRGB view.
  GUP-467: resvg's coverage failed per-pixel tolerance, solved with block
  averages and seeded proofs. Also GUP-429 (naga bans identifiers ending in a
  digit) and GUP-457 (size trigger, accepted for an 83% GPU-pass cut).
- **Gates held.** No `--no-verify`, goldens read by eye, the browser smoke test
  and dogfood in CI, and budgets re-recorded with reasons.

### What recurred

| Problem                    | Evidence                                                                                                                                                                                                                                                                          | Cost                                               |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| Semantic merge conflicts   | GUP-426 default legends broke GUP-431's tests and doubled the hand-placed legends in the WASM scenes (CI red); GUP-494 vs 479 (2- vs 3-wide axis arrays); add/add in `lib.rs`, `plot.rs`, the prelude, `PERF_BUDGETS.md`, `Cargo.lock`; story ID collisions (GUP-439 renumbered). | Two red CI runs, re-work rounds, orchestrator time |
| Budget drift               | `wasm.scatter` re-recorded 15 times. After GUP-451's cut it went 295,904 → 334,149 (+13%) in six days, each step inside +2%. `per_row_type` 941 → 2,569 before it became a real budget; `link_create` 0.93 → 1.11 ms.                                                             | Headroom 136 KB → 98 KB                            |
| Flaky timing gates         | `perf-budget` timing rows failed under multi-agent load in most worker runs; GUP-434 reported a self-contradictory result; CPU medians vary 0.74–0.97 ms with machine state; the browser test's hard frame limits flaked under load.                                              | Re-runs, distrust of real regressions              |
| Skipped checkpoint commits | The limit hit GUP-446 and GUP-454 with no commits and GUP-481 uncommitted; GUP-425 landed as one commit. Partly caused by `all-fix` running fmt before clippy (fixed).                                                                                                            | Risk of lost work                                  |
| Environment                | `LD_LIBRARY_PATH` broke Chromium (GLIBC); windowed dogfood needs the lavapipe ICD, `WAYLAND_DISPLAY` unset and a 1920x1080 Xvfb screen; a doctest saw "multiple versions of crate wgpu" with the shared build dir; a detached test picked a yanked crate offline.                 | Hours per incident, false FAILs                    |
| Token limit                | Weekly limit hit after ~5 days (10-11, on Opus) and ~4 days (10-19, on Sonnet with 3–4 agents in parallel). Each stall left agents mid-story; resumes worked but cost context.                                                                                                    | 9 of 20 days idle                                  |

Two further trends: Sonnet reads goldens but misses subtle numeric errors (the
GUP-426 ramp ticks 0, 2, 4, 5, 6, 8), so the orchestrator must keep zooming into
guides; and workers skip final checks they find slow (perf-budget, old-path-loc)
or claim commits the hook refused. The orchestrator's own tokens (merges,
conflict resolution, re-recording) are not in the ledger, and its memory file
has grown to 380 lines of log loaded every session.

### Recommended process changes

1. **Pace to the allowance.** At most two build-heavy agents at once; Opus
   stories early in the week; a docs-only agent may run as a third. A steady
   pace that never stalls beats a fast one that idles 45% of the time.
2. **Hot-file serialisation.** Stories declare `touches:` in frontmatter
   (`plot.rs`, `lib.rs`/prelude, `encoding.rs`, `PERF_BUDGETS.md`, the WASM
   scenes). Two stories sharing a hot file never run in parallel. The scribe
   allocates IDs from the orchestrator's range (GUP-383's script would enforce
   it).
3. **One post-merge check.** A `mask merge-check` script (gup-core tests,
   `wasm-size`, browser smoke, dogfood) after every multi-branch merge,
   replacing the remembered lesson.
4. **Budgets anchored, timings advisory.** Keep exact counters as hard gates.
   Give `wasm.over_wgpu` a per-track allocation from a fixed anchor (for
   example, 320 KB for release 0.1), so +2% steps cannot ratchet. Every
   re-record cites `mask wasm-attrib` output. Timing rows are `info` in worker
   runs and gated only by an orchestrator run that refuses to start above a load
   threshold.
5. **Checkpoint script.** Adopt the pending `scripts/checkpoint.sh` proposal
   (snapshot to `refs/checkpoints/<branch>`, no hook, HEAD untouched) and call
   it from the worker loop every 30 minutes.
6. **Environment in scripts, not briefs.** A `scripts/headless_env.sh` (or
   `mask env-doctor`) sets and checks the ICD, Xvfb size, `WAYLAND_DISPLAY` and
   `LD_LIBRARY_PATH`, used by `run_all.sh`, `wasm-browser` and the smoke tests.
7. **Measure the orchestrator.** Record its tokens per story in the ledger; move
   the memory log to an archive file and keep a ≤60-line live state.
8. **Shrink what agents must read.** Archive the 346 completed and 29 parked
   legacy stories out of `INDEX.md` (1,693 lines), and split RFC-001 (3,778
   lines) into the design and a findings log. Every story pays for these reads.

## 3. Backlog triage

| Item                                                                                                                                                            | Action                          | Note                                                                                                                                     |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| GUP-439 translucent line joins                                                                                                                                  | **Merge → N2 charts**           | Same polyline geometry as line curves and area outlines; do them together.                                                               |
| GUP-492 windowed size-order draws                                                                                                                               | **Merge → N5 performance**      | Narrow case (windowed + size-ordered + unordered sizes); the real fix is a GPU sort or index buffer.                                     |
| GUP-428 shared units prune                                                                                                                                      | **Keep parked**                 | Trigger: the next toolchain or `Cargo.lock` bump, or a disk alert.                                                                       |
| End-bar padding on numeric axes                                                                                                                                 | **Merge → N2 bar completeness** | With `group_padding`, horizontal bars and value labels: one bar story.                                                                   |
| trybuild `.stderr` churn                                                                                                                                        | **Merge → N0 gates hygiene**    | Narrow the compile-fail cases so new public types do not change rustc's candidate lists, or normalise them.                              |
| `GUP_EGUI_SCREENSHOT`-gated test                                                                                                                                | **Merge → N0 gates hygiene**    | Run it in the `integrations` job under Xvfb, or report it as skipped. A test that passes trivially is a false gate.                      |
| Doctest "multiple versions of crate wgpu"                                                                                                                       | **Merge → N0 gates hygiene**    | Reproduce; if CI is clean, it is a shared-build-dir hazard to fix in `cargo_env.sh`.                                                     |
| `group_padding`, horizontal bars, line curves, area, bar value labels                                                                                           | **Merge → N2 charts**           | Value labels need a text mark first.                                                                                                     |
| Dogfood formats dropped (t1 `%Y-%m`, t3 `$Nk`)                                                                                                                  | **Merge → N0**                  | `tick_format` (GUP-474) now exists; restore the original intent.                                                                         |
| Stale guides: GRID_SYSTEM, LOD_SYSTEM, CUSTOM_MARK_GUIDE, TYPE_CONSTRUCTION_GUIDE, PERFORMANCE_GUIDE, `mark-system/*`                                           | **Drop** (delete)               | Git keeps them. CUSTOM_MARK_GUIDE is rewritten in N1 around `#[derive(Mark)]`; PERFORMANCE_GUIDE becomes a pointer to `PERF_BUDGETS.md`. |
| Accessibility docs (8 guides, `docs/accessibility/`)                                                                                                            | **Archive**                     | Move to `docs/archive/`; N4 starts from them.                                                                                            |
| `tutorials/*`                                                                                                                                                   | **Merge → N1 docs**             | Rewrite as tested Markdown (the GUP-489 doctest-include pattern).                                                                        |
| `research/*`, `existing-solutions-analysis.md`                                                                                                                  | **Archive**                     | Historical.                                                                                                                              |
| `gup-core` crate docs ("one mark, one layer", "not re-exported until S14", `RenderContext`, SVG "guides only") and its Cargo description ("new core (RFC-001)") | **N0, first story**             | Small and user-facing: docs.rs shows it. Also `CLAUDE.md` says wgpu v26; the workspace is on 27.                                         |
| Parked `gup-ios`, `gup-android`, Tauri (+ `ios-ci.yml`, `android-ci.yml`)                                                                                       | **Drop** (Decision 3)           | They reference deleted features and APIs; a re-wire is a rewrite.                                                                        |
| Legacy open stories (GUP-285, 286, 305, 308, 309, 312–315, 350, 358, 359, 370–373, 274, 354, 378, 285B, 375, 402)                                               | **Close: obsolete**             | They target code deleted in GUP-486/488. GUP-312–315 and 370–373 seed N5 if still relevant.                                              |
| GUP-392, GUP-404                                                                                                                                                | **Close / rescope**             | 392 is delivered by `gup-core` (Inter, Okabe-Ito, step-derived ticks); verify and close. 404: 9 `#[ignore]`s remain, fold into N0.       |
| GUP-383 (ID guard), 397 (shared visual assertions), 376 (`on_key`)                                                                                              | **Keep**                        | 383 into N0 (ID collisions recurred); 397 into N0; 376 into N4 (keyboard).                                                               |
| GUP-405 shaping, GUP-416 font fallback                                                                                                                          | **Keep → N2**                   | Tabular figures and kerning are publication quality.                                                                                     |

## 4. Proposed next tracks

Ordered. "Size" is in Sonnet-story equivalents (about 150–250K tokens each,
including scribe share and fix rounds).

| #   | Track                | Size      | Content and rationale                                                                                                                                                                                                                                                                                                                                                                             |
| --- | -------------------- | --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| N0  | Hygiene              | S, 4–6    | Fix the `gup-core` crate docs; delete or archive the stale docs; archive legacy INDEX and close obsolete stories; gates hygiene (trybuild, egui screenshot, wgpu doctest, `headless_env.sh`, `merge-check`, `checkpoint.sh`, GUP-383 ID guard); restore the dogfood formats. Cheap, mostly docs, and it lowers the cost of every later story.                                                     |
| N1  | Docs and tutorials   | M, 5–7    | Six tutorials as tested Markdown; a `#[derive(Mark)]` guide; `#![warn(missing_docs)]` on `gup-core`'s public items; README claims cut to what is measured. Files are disjoint from code, so N1 runs beside N2 at low conflict risk.                                                                                                                                                               |
| N2  | Chart vocabulary     | L, 10–14  | Text mark (annotations, value labels); area and stacked area; horizontal bars plus bar completeness; histogram (bin transform); heatmap builder (`Rect` + band + sequential); line curves with GUP-439; subtitle and a dark theme; GUP-405/416 text quality. Then box plot. This is the gap between "a demo" and "my chart".                                                                      |
| N3  | Release 0.1          | M, 4–6    | Licence (Decision 1); publish metadata, versions on path dependencies, descriptions without RFC jargon; name check; `cargo publish --dry-run` and a docs.rs build in CI; a Windows (WARP) render smoke job, macOS if a runner GPU works; CHANGELOG and a 0.x semver policy; minimal accessibility (Decision 4); flagship gallery of 8–10 CI-rendered charts and a README hero image (October T6). |
| N4  | Accessibility        | M–L, 6–10 | `Plot::describe()` from the resolved scene; SVG `<title>`/`<desc>` and `role="img"`; a canvas `aria-label` and hidden data table on the web; keyboard focus and arrow-key navigation over picks (GUP-376); a high-contrast theme. Starts from the archived docs.                                                                                                                                  |
| N5  | Performance at scale | L, 8–12   | Columnar input (`from_columns` reachable from `Plot`, Arrow behind a feature) so rows skip CPU closures; port LOD and density aggregation from `gup-culling-lod`; GPU sort (GUP-492); a 100M-row and then 1B-row demonstration with budgets. Only after this may the README say "billion".                                                                                                        |
| N6  | Export and platforms | S–M       | PDF via `svg2pdf` over `SvgTarget` (S, on demand); mobile and Tauri rebuilt from scratch on `gup-core` only when a user asks.                                                                                                                                                                                                                                                                     |

At two agents in parallel and the observed allowance, N0 to N3 is about three
weeks of steady work. N4 and N5 are the next review's question.

## Decisions for the owner

1. **Licence for the first release.**
   - (a) Keep GPL-3.0-or-later: every application that links Gup must be GPL.
   - (b) MIT OR Apache-2.0, as wgpu, egui and Bevy are.
   - (c) MPL-2.0: file-level copyleft; changes to Gup stay open, applications do
     not.
   - **Recommendation: (b)** if adoption is the goal, (c) if copyleft matters.
     Decide before N3, because every file's header changes.
2. **What 0.1 is.**
   - (a) Publish after N0: three chart types, honest README.
   - (b) After N0–N2 plus N3: the common chart vocabulary, tested tutorials, a
     gallery.
   - (c) After N5 as well: lead with scale.
   - **Recommendation: (b).** (a) spends the first impression on a narrow crate;
     (c) delays any outside feedback by a month or more.
3. **Parked mobile and Tauri crates.**
   - (a) Keep parked (October Decision 3).
   - (b) Delete them and their workflows; git keeps them.
   - (c) Re-wire now.
   - **Recommendation: (b).** They name deleted features and APIs, so a re-wire
     is a rewrite either way, and parked code goes stale silently.
4. **Accessibility scope for 0.1.**
   - (a) None; N4 after release.
   - (b) Minimal in 0.1: `Plot::describe()`, SVG title and description, canvas
     `aria-label`; full N4 after.
   - (c) Full N4 before release.
   - **Recommendation: (b).** It is small, and a chart library without a text
     alternative is excluded from many publications and public-sector sites.
5. **Pace and safety net.**
   - (a) Keep 3–4 agents in parallel.
   - (b) At most two build-heavy agents, Opus early in the week, hot-file
     serialisation, and adopt `checkpoint.sh`.
   - (c) Fully serial.
   - **Recommendation: (b).** The limit, not agent quality, cost 9 of 20 days;
     (b) trades some peak throughput for no stalls and fewer semantic conflicts.

## Owner decisions (2026-10-24)

1. **Licence:** keep GPL-3.0-or-later. No header change.
2. **0.1 scope:** (b), publish after N0 to N2 plus N3: the common chart
   vocabulary, tested tutorials and a gallery.
3. **Mobile and Tauri crates:** (b), delete them and their workflows (git keeps
   them).
4. **Accessibility in 0.1:** (b), minimal: `Plot::describe()`, an SVG
   title/description and a canvas `aria-label`. Full N4 comes after release.
5. **Pace:** (b), at most two build-heavy agents, Opus early in the week,
   hot-file serialisation, and adopt `scripts/checkpoint.sh` (a work-in-progress
   snapshot to a private ref: no hook, never pushed, never moves the branch).
