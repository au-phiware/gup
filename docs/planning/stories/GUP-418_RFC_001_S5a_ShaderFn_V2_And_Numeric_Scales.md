# GUP-418: RFC-001 Step S5a: ShaderFn v2, `then` Composition and the Numeric Scale Family

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-11)
**Created**: 2026-10-10

## Context

[RFC-001](../rfcs/RFC-001_Core_Architecture.md) §5 defines one scale family:
every scale is a `ShaderFn` with an exact f64 `CpuMirror`, so axes, ticks,
legends, picking and the GPU all agree. §11's story table lists this as step S5:
"`ShaderFn` v2 + encase + `then`; full scale family with CPU mirrors; GPU≡CPU
conformance harness." [GUP-395](GUP-395_Gup_Core_Vertical_Slice_Headless.md)
(S0a) built `Linear`, `Log` and `Sequential`, each a `ShaderFn` with `params()`,
`chunk_base()` and `fit_domain()` (the S0a findings' refinement of §5's single
`params(&self, cx: &ParamCx)`), and the `conformance` test module that
dispatches a scale's WGSL in a compute pass and checks it against its
`CpuMirror` within 0.25 px (or 1/255 for colour).
[GUP-415](GUP-415_RFC_001_S4b_Nulls_And_Dictionaries.md) (S4b) added a minimal
`Categorical` stand-in, explicitly documented as "a placeholder for RFC-001 S5's
full scale family."

This story is the first of three that complete S5 (split per the task that
created it, since S5 as written covers more than one focused session):

- **S5a (this story)**: the `then` composition primitive and the position scales
  that need no dictionary — Linear (done), Pow/Sqrt, Log (done) with Symlog, and
  Time, plus the `Time` precision fix S4a's findings left open.
- **S5b** ([GUP-419](GUP-419_RFC_001_S5b_Colour_Scales_And_Dictionary_Hook.md)):
  colour scales (Sequential done, Diverging, the full `Categorical`), the
  dictionary hook, and the ordinal position scales (Band, Point) that need it.
- **S5c** ([GUP-420](GUP-420_RFC_001_S5c_Wgsl_Function_Macro_V2.md)):
  `#[wgsl_function]` v2, so a user can author a new `ShaderFn` the same way the
  library's own scales are authored, with the build-time composition GUP-406
  moved every library module onto.

**Composition.** §5 says: "`a.then(b)` (`Then<A, B>` where `A::Out == B::In`)
emits `b(a(x, pa), pb)` in the glue module. Each `Params` value is a separate
field in a packed per-layer `Encodings` uniform." Today's glue emitter
(`crates/gup-core/src/shader/glue.rs`) gives each channel exactly one
`ChannelSource::Column(&dyn DynShaderFn)`, which becomes exactly one
`Expr::Call`. `then` needs a channel to carry a _chain_ of calls, each
contributing its own field to `Encodings` and its own line to the pipeline cache
signature, with the chain's first link alone taking the per-chunk relative base
(only the column's own storage format is relative; every later link in the chain
consumes the previous link's GPU output, which is never a stored column). This
is the "v2" in `ShaderFn` v2: the trait itself
(`params`/`chunk_base`/`fit_domain`/`resources`) is close to right after S0a,
but the glue emitter's one-call-per-channel assumption is not, and needs
generalising to a chain before `then` can exist.

**Time precision.** RFC-001 §12 risk 6 anticipated this: "f32 precision on deep
zoom inside a chunk... chunk origins cover timestamps and typical zoom ranges.
Add an opt-in `F32x2` (hi/lo, 'df64') column format later." S4a's findings
measured the actual failure: a chunk's precision is bounded by its _value span_,
not merely by being relative. A full default chunk of one-per-second samples
spans about 12 days; a one-second, 1000 px zoom near the end of that chunk
misses the 0.25 px budget by about 40 px (the
`conformance::chunk_boundary::a_full_chunk_of_seconds_misses_a_one_second_zoom`
test records this, asserting the miss as a known, not-yet-fixed limit). S4a's
findings list three options to decide between in S5: "cap a time column's chunk
by value span... a hi/lo f32 pair (double-single) column format, or origins per
sub-chunk," and note the decision "can reuse `conformance::chunk_boundary` (it
binds uploaded chunk columns)."

**Recommendation: cap chunk value span for `Time` columns, not a new column
format.** `ColumnStore::chunk_rows_for` (GUP-414/S4a) already turns a per-row
byte budget into a row-count cap; this story extends it (for formats that
declare one) with a _value-span_ cap that stops the current chunk and opens a
new one — with a fresh origin — once the running value would be more than a
fixed span from the chunk's origin. This is recommended over the alternatives
because:

- It reuses the per-chunk-origin mechanism S4a already built and proved at
  `conformance::chunk_boundary`, instead of adding a second, parallel precision
  strategy (`F32x2`) next to it — the project's own history (four scale systems,
  three composition systems) is why RFC-001 exists, and a second precision
  mechanism living beside the first repeats that pattern.
- It needs no new `ColumnFormat`, no new WGSL arithmetic (double-single add/sub,
  which every consuming entry point would need), and no change to `encase`'s
  `Params` layout — `F32Relative` stays exactly as S0a/S4a built it.
- The chunk count it produces is bounded by the row count (a chunk never splits
  below one row), so pathologically sparse-but-wide-ranging data costs draw
  calls, not correctness, and real time-series data dense enough to need a
  one-second zoom is also dense enough that a 4,000 s (say) span cap does not
  explode the chunk count in practice.
- Sub-chunk origins (storing more than one origin per chunk, selected by row)
  would avoid the extra draw calls, but needs a second indexed lookup in every
  relative entry point's call site and a decision about how many origins per
  chunk — more moving parts for a saving (buffer count) that `zoom_bench`'s S4a
  numbers show costs nothing measurable (seven chunks cost the same as one). The
  hi/lo pair stays what §12 risk 6 already called it: a future, opt-in column
  format if span-capped chunking turns out not to be enough for some real
  workload.

This story picks the span cap and proves it against
`conformance::chunk_boundary`'s existing fixtures (extended with a positive
case: the same "full chunk" scenario, now capped, passing within budget) as the
primary evidence; if the implementer finds the span cap insufficient or
impractical for a case `chunk_boundary` exposes, record that finding in RFC-001
and revisit before building the hi/lo format, rather than building both.

## User Story

> "As a visualization developer, I want `x_scale.then(color_scale)`-style
> composition and a full numeric scale family (linear, power, square root,
> logarithmic, symmetric log, and time with calendar ticks) that stays within
> 0.25 px of its CPU mirror at every chunk boundary and at deep zoom, so that I
> can build charts whose positions are correct at any scale without writing GPU
> code myself."

## Acceptance Criteria

### AC1: `then` composition

- [x] `a.then(b)` (where `A::Out == B::In`) produces a value usable wherever a
      single `ShaderFn` is today (as a channel's encoding function), and the
      generated glue calls `b`'s entry on `a`'s result in one expression, per
      §5.
- [x] Each link in a chain contributes its own `Params` field to the generated
      `Encodings` struct (not a merged/flattened struct), and the pipeline cache
      signature names every link.
- [x] Only the chain's first link (the one consuming the stored column) can take
      a per-chunk relative base; a test using a `F32Relative` first link and a
      `Then` second link confirms the base is applied once, correctly.
- [x] A test chains at least three scales/colour functions
      (`linear.then(sqrt_like).then(sequential)` or an equivalent real
      three-link chain) and checks the GPU result against evaluating the CPU
      mirrors in the same order, within the position/colour tolerance.
- [x] An external-crate doctest on `gup-core`'s public API builds and renders a
      chart using `.then(..)` on at least one channel.

### AC2: Pow/Sqrt scale

- [x] A `Pow` scale (with `Sqrt` as `Pow::sqrt()` or equivalent) implements
      `PositionScale` and `CpuMirror`, with a configurable exponent.
- [x] `conformance`'s GPU≡CPU harness covers `Pow`/`Sqrt` within 0.25 px,
      including near-zero and negative-domain inputs if the scale supports them
      (document the domain restriction if it does not).
- [x] A golden/structural render (read by eye, and checked through the
      `gup-visual-regression` harness's perceptual tolerance — never a
      byte-exact comparison to a PNG blessed only on this machine) shows a
      scatter with `Circle::RADIUS` driven by a `Sqrt` scale (the RFC's own
      north-star example: size by population) with visibly non-linear radius
      growth and every mark inside the plot rect.

### AC3: Log gains Symlog

- [x] `Log` supports (or a new `Symlog` type sharing its WGSL module supports) a
      symmetric-log mode that is linear through zero and logarithmic beyond a
      configurable linear threshold, per §5.
- [x] `conformance` covers `Symlog` within 0.25 px across a domain that
      straddles zero, including points inside and outside the linear threshold.
- [x] A render with a y-domain that crosses zero (e.g. a signed quantity) using
      `Symlog` shows points on both sides of zero without a gap or a
      discontinuity jump at the threshold, verified by eye.

### AC4: `Time` scale and the precision fix

- [x] `Time` implements `PositionScale` and `CpuMirror` as "Linear over f64
      seconds," per §5, accepting a numeric (Unix-seconds-like) accessor — this
      story does not require a `chrono`/calendar-type accessor, only
      calendar-aware **ticks**.
- [x] `Time::ticks` produces calendar-aware tick positions and labels (for
      example, ticks that land on whole days, months or years depending on the
      domain span), not merely evenly spaced numeric ticks.
- [x] **Precision approach chosen by a measured spike, not by argument**
      (orchestrator amendment, 2026-10-10). The Context recommends value-span
      capping, but the arithmetic is unfavourable at scale: with f32's 24-bit
      mantissa, 0.25 px at a 1 s / 1000 px zoom allows a chunk span of only
      about 0.25/1000 × 2²⁴ ≈ 4,200 s. Three years of one-per-second data would
      then need about 22,500 chunks, so 22,500 draw calls per frame, and the cap
      shrinks with zoom depth (≈4 s at 1 ms / 1000 px). S4a only measured 7
      chunks. Before implementing either approach, spike both: 1.
      value-span-capped chunks; 2. a hi/lo f32 pair column format
      (double-single), a new `ColumnFormat` beside `F32Relative`, not a parallel
      system.

      Measure each on two scenarios:
      - (a) three years of one-per-second samples at a 1 s / 1000 px zoom;
      - (b) the same data at 1 ms / 1000 px.

      Record the chunk/draw count, `zoom_bench` CPU and GPU frame time, column
      bytes and WGSL cost, and precision against the CPU mirror. Choose the
      approach that meets 0.25 px in both scenarios within the S0b frame
      budget. If span capping needs more than about 64 chunks in scenario (a),
      or fails (b), use the hi/lo format. Record the numbers and the decision
      in RFC-001's findings.

- [x] The chosen approach makes `conformance::chunk_boundary`'s "full chunk of
      one-per-second samples, one-second zoom" scenario pass within 0.25 px
      (replacing or adding to the existing test that documents the miss as a
      known limit), and a new 1 ms-zoom variant also passes.
- [x] The existing `conformance::chunk_boundary` tests
      (`relative_chunks_stay_within_a_quarter_pixel_at_the_boundary`,
      `absolute_f32_misses_at_the_boundary`,
      `one_relative_chunk_spanning_years_misses`) still pass unchanged, proving
      the span cap does not regress the already-proven multi-chunk and
      negative-control behaviour.
- [x] A golden render shows a line or scatter chart with a `Time` x-axis across
      a multi-year domain with calendar tick labels (e.g. year boundaries), read
      by eye through the harness tolerance.
- [x] A second golden or structural test zooms a `Time`-scaled chart to a
      sub-second window inside a large dataset and confirms (via the
      `Layout`/pick machinery or a direct position check, not merely "looks
      right") that rendered positions match the CPU mirror within 0.25 px at
      that zoom level.

### AC5: Multi-workgroup correctness guardrail

- [x] Domain fitting for every scale in this story continues to use the
      `ColumnStore`'s existing CPU f64 chunk stats (per RFC-001 §3); no new GPU
      compute reduction is introduced for this story's scales.
- [x] If the implementation does introduce any GPU compute-shader reduction (for
      example, to accelerate domain fitting at very large row counts), it is
      proven correct when dispatched across more than one workgroup — with a
      dedicated test that forces a multi-workgroup dispatch, not just a
      single-workgroup-sized input — recording explicitly why the old path's
      `compute_basic_stats` was racy and how this test rules out the same class
      of bug. This AC exists as a guardrail regardless of whether this story
      ends up needing it.

### AC6: Browser and old-path freeze

- [x] `mask wasm-browser` is extended to exercise `then` composition, `Pow`/
      `Sqrt`, `Symlog` and `Time` (at minimum one scene using each), and passes
      on WebGPU/SwiftShader.
- [x] No file outside `crates/gup-core` (and planning docs) changes;
      `mask old-path-loc` reports the same count as before this story. (28,910
      before and after. The three new goldens live in `tests/golden/gup_core/`,
      where every gup-core golden lives; they are the only files outside
      `crates/gup-core` and `docs/planning`.)

## Technical Tasks

- [x] Generalise the glue emitter's channel source from "one `DynShaderFn`" to
      "a chain of `DynShaderFn`s," threading each link's output as the next
      link's input, emitting one `Encodings` field and one pipeline-signature
      fragment per link, and applying the per-chunk relative base only to the
      first link.
- [x] Add a `Then<A, B>` (or equivalently named) combinator and wire it through
      `IntoEncoding` so `a.then(b)` type-checks like any other `ShaderFn`-backed
      encoding.
- [x] Implement `Pow`/`Sqrt` as a `PositionScale` + `CpuMirror`, with its WGSL
      module under `src/shaders`.
- [x] Extend `Log`'s WGSL/Rust (or add `Symlog`) for the symmetric-log mode.
- [x] Implement `Time` as a `PositionScale` + `CpuMirror` over f64 seconds, with
      calendar-aware `ticks()`.
- [x] ~~Extend `ColumnStore`'s chunking with a value-span cap~~ Replaced by the
      spike's decision: a hi/lo `ColumnFormat::F32x2Relative` column, read by
      `Time` (span capping needs ~46,200 chunks in scenario (a) and ~19M in
      (b)). See RFC-001 "S5a findings".
- [x] Extend `conformance.rs` and `conformance::chunk_boundary` with
      `Pow`/`Sqrt`, `Symlog` and `Time` cases, including the positive, hi/lo
      "full chunk" case.
- [x] Add the `then`-chain GPU≡CPU and pipeline-signature tests (AC1).
- [x] Add the guardrail test or documentation note for AC5.
- [x] Extend the `mask wasm-browser` harness scene(s) for AC6.
- [x] Re-run `mask old-path-loc` and confirm it is unchanged.

## Dependencies

### Prerequisite Stories

- GUP-395 (S0a) ✅ — `Linear`/`Log`/`Sequential`, `ShaderFn`, and the
  `conformance` GPU≡CPU harness this story extends.
- GUP-414 (S4a) ✅ — the multi-chunk `ColumnStore` with per-chunk origins and
  stats, and `conformance::chunk_boundary`, which this story's `Time` precision
  fix builds on directly.
- GUP-410 ✅ (soft) — GPU error scopes already wrap pipeline creation; new scale
  pipelines and the `then`-chain glue must keep running inside them.

### Enables Stories

- [GUP-419](GUP-419_RFC_001_S5b_Colour_Scales_And_Dictionary_Hook.md) 📋 (S5b) —
  shares this story's `ShaderFn` v2 / chained-glue shape, and the pipeline cache
  signature convention this story establishes for multi-link channels.
- [GUP-420](GUP-420_RFC_001_S5c_Wgsl_Function_Macro_V2.md) 📋 (S5c) — the macro
  emits an impl of the `ShaderFn` trait this story finalises (`params`/
  `chunk_base`/`fit_domain`/`resources`, and chain-compatibility), so S5c
  depends on this story's trait shape being settled.
- RFC-001 S7 (`Theme`, guide emitters; not yet written) — axis tick rendering
  for `Time`'s calendar ticks is input for the guide emitter's label-density and
  formatting work.

## Testing Strategy

- **Unit tests**: `Pow`/`Sqrt`/`Symlog`/`Time` CPU mirrors against known values;
  `Time::ticks` calendar boundaries at several domain spans; `ColumnStore`'s
  value-span chunk split at and around the cap.
- **Conformance tests**: GPU≡CPU within 0.25 px for every new scale and for
  `then` chains, reusing and extending `conformance.rs` and
  `conformance::chunk_boundary`.
- **Golden/visual tests**: read by eye through the `gup-visual-regression`
  harness's perceptual tolerance (never byte-exact against a PNG blessed only on
  the implementer's machine) for AC2 (Sqrt radius), AC3 (Symlog straddling zero)
  and AC4 (Time calendar ticks, and a deep-zoom position check).
- **Doctest**: an external-crate-style doctest exercising `.then(..)` (AC1).
- **Browser**: `mask wasm-browser` extended per AC6.

## Success Metrics

- [x] `then` composition, `Pow`/`Sqrt`, `Symlog` and `Time` all pass their
      GPU≡CPU conformance checks within 0.25 px (or 1/255 for colour, where
      applicable).
- [x] The S4a "full chunk of seconds, one-second zoom" miss is fixed (not merely
      documented) for `Time` columns, proved against
      `conformance::chunk_boundary`.
- [x] `mask wasm-browser` and `mask old-path-loc` are unaffected beyond the
      intended scene additions.

## Risk Assessment

- **Medium**: generalising the glue emitter to chains of calls is a real
  architectural change to code every later S5/S6 story also touches.
  _Mitigation_: keep the single-link case (today's only case) byte-identical in
  its generated WGSL and `Encodings` layout — a regression test diffs the
  existing fixtures (`scatter_glue.wgsl`, S0a's layout tests) before and after
  this change.
- **Medium**: the value-span cap is a new per-format policy inside
  `ColumnStore`, which GUP-414/415 already made sensitive to byte-exact layout
  assumptions (4-byte strides, 64-row blocks). _Mitigation_: gate the cap behind
  an explicit per-format opt-in so `Linear`/`Log`/`Sequential`'s existing
  chunking, byte counts and fixtures are provably unchanged (a test asserts
  this), and only `Time`-formatted columns pay for the new check.
- **Low**: calendar tick generation (month/year boundaries, varying month
  lengths) is a known source of off-by-one bugs. _Mitigation_: unit-test
  `Time::ticks` at domain spans crossing a leap year, a month with 28–31 days,
  and a year boundary, each checked against hand-verified expected tick
  positions.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1` on Intel (and
      the lib, doc and `scales_png` tests on lavapipe). The root crate's tests
      are left to CI: the orchestrator ruled out building them locally (disk).
- [x] Lint and format clean: `cargo fmt --all --check`, gup-core clippy
      (`--all-targets --all-features -D warnings`), `scripts/clippy_wasm32.sh`
      and the pre-commit hook on every commit. The full `mask all-check` is left
      to CI, for the same disk reason.
- [x] All examples compile: `cargo check -p gup-core --examples` (gup-core only,
      as above)
- [x] Rendered output verified by eye (golden image or PNG read) for AC2, AC3
      and AC4
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

**Precision spike and decision: hi/lo.** The spike's numbers are in RFC-001 "S5a
findings".

- A value-span cap must be 2,048 s at a 1 s / 1000 px zoom (4,096 s misses at
  0.26 px) and 4 s at 1 ms (0.24 px). For three years of one-per-second data
  that is about 46,200 chunks in scenario (a) and about 18.9M in (b).
- A `zoom_bench` proxy with 50,000 draws (100K points) spends 95 ms of CPU per
  frame and 12.8 MB of chunk uniforms. Scenario (b)'s chunk uniform would be
  larger than a buffer may be.
- The hi/lo `ColumnFormat::F32x2Relative` is within 4.4e-5 px at every chunk
  span (up to 1e8 s) and both zooms, on Intel and lavapipe. It needs 91 chunks
  in both scenarios, which is `MAX_CHUNK_ROWS`, not precision.
- hi/lo costs nothing measurable on the GPU (3.83–3.98 against 3.95–4.13 ms), a
  few hundredths of a ms of CPU, and 4 B per row.
- The story's rule picked hi/lo, and the spike's numbers are committed
  (`dc7354e`) before any of the rest was built.

**Delivered** (all in `crates/gup-core`, plus three goldens):

- **ShaderFn v2 and `then`** (`src/encoding.rs`, `src/shader/glue.rs`,
  `src/selection.rs`):
  - `ShaderFn` is one WGSL function. `chunk_base` now returns f64, split by the
    column format, and there is a new `image` (the output extent).
  - `EncodeFn` is what a channel encodes through: a `ShaderFn` or a
    `Then<A, B>`. `encode`, `encode_key` and `then` live on it.
  - `Feeds` is the link type rule (a `Px` output feeds an `f32` input).
  - `CpuMirror` is over `EncodeFn`, so chains have mirrors.
  - The glue emits a chain per channel: an `Encodings` field per link (`<ch>`,
    `<ch>_link<k>`), `then`-joined signatures and LUTs per link. Only the first
    link's base comes from the chunk origin; later relative links get origin
    0's. Single-link glue is byte-identical, and the fixtures are unchanged.
  - Later links' auto domains fit to the image of the data.
  - Plots inset their ranges by an encoded `Size` channel's largest output.
- **`ColumnFormat::F32x2Relative`** (`src/column.rs`): 8-byte stride, fetched as
  `Float32x2`, with hi/lo chunk bases in the `Chunk` uniform.
- **Scales** (`src/scale/{pow,symlog,time}.rs`, `src/shaders/scale_*.wgsl`):
  - `Pow`/`Pow::sqrt()`: sign-keeping power, positive exponents.
  - `Symlog`: smooth `log2(1 + |x|/c)` with a near-zero series, and decade ticks
    across zero.
  - `Time`: linear over Unix seconds through `gup::scale::time` (hi/lo), with
    UTC calendar ticks and calendar `nice`.
- **Tests**:
  - Conformance: Pow/Sqrt ≤ 2.7e-3 px, Symlog ≤ 1.2e-4 px, a three-link chain ≤
    2.3e-5 per colour channel, the base applied once (a seeded second base is
    off by 4.7e9 px), hi/lo full chunk at 1 s and 1 ms (7.0e-4 and 4.4e-5 px),
    and the S4a `F32Relative` miss kept as the negative control (39.9 px).
  - Glue and layout tests for chains, a trybuild case for a mismatched chain,
    and an external doctest on `EncodeFn::then`.
  - Calendar unit tests: leap day, 28–31-day months, year boundaries, hours to
    sub-milliseconds.
  - `tests/scales_png.rs`: three goldens plus the sqrt-area, symlog and 1 ms
    deep-zoom pixel checks.
  - Test counts: gup-core lib 108 passed (3 ignored), doc 7, integration 21 (5
    new in `scales_png`).
- **Browser**: a third `mask wasm-browser` scene (`Time`, `Symlog`, `Sqrt`, a
  three-link `then` fill) with in-wasm mirror checks. It passes on SwiftShader,
  with the 1 ms zoom at 0.027 px.
- **`zoom_bench --time`** encodes x through `Time`.
- **`PERF_BUDGETS.md`** re-records the WASM scatter: +17,807 B gz for the new
  scene, +974 B for the library.

### Definition-of-Done evidence

- **Rendered output, read by eye.**
  - `gup_core/sqrt_radius`: 60 countries, GDP against life expectancy, discs
    from barely visible to 28 px whose areas follow population, viridis by life
    expectancy, every disc inside the plot rect.
  - `gup_core/symlog_signed`: an S-curve of `i³/8` from −216,000 to 216,000.
    Ticks run −1M … −10, 0, 10 … 1M, uncrowded at zero. Points are continuous
    through zero.
  - `gup_core/time_years`: a daily series from mid-2019 to late 2023, with x
    ticks `2019, Jul, 2020, Jul, …, 2024` on 1 January and 1 July.
  - The browser PNG's third panel: hourly points labelled
    `Nov 15, 06:00, 12:00, 18:00, Nov 16` on a symlog y axis, sized by sqrt and
    coloured by the chain.
  - On lavapipe the goldens differ by 0 of 324,000 pixels (max ΔE 1.6).

**`mask perf-budget`**: every check passed in its final run, at GPU clock median
483 MHz (the floor state):

| Metric                        | Measured | Budget      |
| ----------------------------- | -------: | ----------- |
| `zoom.gpu_pass.median_ms`     |    3.530 | ≤ 4.668     |
| `zoom.gpu_pass.p95_ms`        |    6.409 | ≤ 7.875     |
| `zoom.cpu.median_ms`          |    0.806 | ≤ 0.912     |
| `zoom.cpu.p95_ms`             |    1.082 | ≤ 1.412     |
| uploads (columns, uniforms)   |  0, 88 B | exact       |
| `pipeline.link_create.median` | 0.856 ms | ≤ 1.056     |
| `pipeline.link_create.cold`   | 1.056 ms | ≤ 10        |
| `wasm.scatter.gz_bytes`       |  271,706 | re-recorded |
| `wasm.over_wgpu.gz_bytes`     |  229,994 | ≤ 400,000   |

Earlier runs:

- At 517 MHz: GPU 3.26 ms, CPU 0.74 ms. It failed only `wasm.scatter`, before
  the re-record.
- At 867 MHz (boosted, so its timing checks are weak): passed.

## Retrospective

**Completed**: 2026-10-11

### Key Technical Learnings

#### The spike's arithmetic was right, and the margin was thinner

- **Challenge**: the story estimated a 4,200 s span cap. The measured cap that
  passes at 1 s / 1000 px is 2,048 s: at 4,096 s the error is 0.26 px, because
  rounding the base and the value each costs half a ULP.
- **Solution**: measure with points at the far end of a chunk, on golden-ratio
  offsets. The first version put points at short binary fractions, whose
  rounding cancelled against the base's, and reported 0 px at 8,192 s.
- **Pattern**: precision tests need inputs that do not share the base's
  rounding. Otherwise a broken store can look exact.

#### hi/lo is lossless for f64 Unix seconds

- **Challenge**: double-single was expected to need careful arithmetic
  (two-sum), and to lose at very sparse chunks.
- **Solution**: f64 timestamps near 1.7e9 are multiples of 2^-22 s, so any
  offset within 2^26 s of the origin fits the pair's 48 bits exactly. Adding the
  high words first cancels exactly for on-screen points (Sterbenz), so plain f32
  adds suffice. Intel/Mesa, lavapipe and SwiftShader/Tint all keep the order.
- **Pattern**: before reaching for a general double-single library, check
  whether the input's own precision already bounds the problem.

#### Associated types across a blanket impl

- **Challenge**: with `CpuMirror: EncodeFn` and
  `impl<S: ShaderFn> EncodeFn for S`, generic code bounded by
  `S: ShaderFn + CpuMirror` sees `<S as EncodeFn>::Output` as opaque: rustc
  prefers the where-clause candidate to the impl. So it cannot equal
  `ShaderFn::Out`.
- **Solution**: give the chain trait differently named associated types
  (`Input`, `Output`), and spell both out where needed
  (`PositionScale: ShaderFn<Out = Px> + EncodeFn<Output = Px>`,
  `S: CpuMirror<Output = <S as ShaderFn>::Out>`).
- **Pattern**: a supertrait over a blanket-implemented trait needs its
  projections named in bounds.

#### `then` is cheap once the glue thinks in links

- **Solution**: the emitter loops over links where it had one call, and the
  per-link names (`<ch>`, `<ch>_link<k>`) keep the single-link output
  byte-identical. The real decision was the base rule: only link 0 reads the
  stored column, so a later relative link's base is computed for origin 0.

### Architectural Decisions

#### hi/lo column for `Time`, not span-capped chunks

- **Decision**: `ColumnFormat::F32x2Relative`, read by `Time`. `Linear` keeps
  `F32Relative`.
- **Reasoning**: the measured rule. Span capping needs 46,200 draws (95 ms of
  CPU a frame in the proxy) or 19M. hi/lo costs 4 B a row and nothing on the
  GPU.
- **Trade-off**: twice the x column bytes for time data, and a second relative
  format to support (validity, chunk bases, glue).
- **Future**: `Linear::precise()` could share the arithmetic. S9's chunk
  culling, not the format, is what makes deep zoom into 94.7M rows cheap.

#### `ShaderFn` (one function) plus `EncodeFn` (a chain)

- **Decision**: keep `ShaderFn` as what one WGSL function (and S5c's macro)
  implements, and put `encode`, `encode_key` and `then` on `EncodeFn`.
- **Reasoning**: `Then<A, B>` has no single module, entry or `Params`, so it
  cannot honestly be a `ShaderFn`.
- **Trade-off**: users import `EncodeFn` (it is in the prelude). The compile
  errors name `EncodeFn::Output`.
- **Future**: S5b and S5c implement `ShaderFn` and get chaining for free.

#### `Symlog` as its own module

- **Decision**: a new `gup::scale::symlog` beside `gup::scale::log`.
- **Reasoning**: adding a mode to `Log` would change its `Params`, layout and
  the linked reference-scatter fixture, against this story's own risk
  mitigation.

### Development Workflow Insights

- **The browser harness is also the WASM size proxy.** A test scene costs size
  budget (+17.8 KB gz here), and the budget had to be re-recorded with the split
  between library and scene stated.
- **Measuring the GPU pass at 100K points with each approach's draw count**
  separated draw-call cost from instance throughput. The full 94.7M-row scenario
  would not fit in the machine's free memory (5 GB), and without culling it
  would only have measured throughput.
- **Disk was tight.** Deleting stale test binaries did not free space until the
  ZFS pool caught up. A heredoc-plus-perl edit that failed to compile once
  truncated a file to empty before a `cp`; it was recovered from git. Use the
  Edit tool for multi-line edits.
- **A first `Symlog` golden used data that is the inverse of the transform.** It
  drew a straight line, which proves nothing. Read the PNG before trusting a
  pass.

### Follow-up Stories

None. The open items are already covered:

- S5b (GUP-419) and S5c (GUP-420) take the adjustments in RFC-001 "S5a
  findings".
- Chunk culling is RFC-001 S9.
- `Linear::precise()` and time-zone or locale-aware `Time` labels are noted
  there and in the RFC. Neither has a workload asking for it yet.
