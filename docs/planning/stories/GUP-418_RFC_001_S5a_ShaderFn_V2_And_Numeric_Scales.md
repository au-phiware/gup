# GUP-418: RFC-001 Step S5a: ShaderFn v2, `then` Composition and the Numeric Scale Family

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-10

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

- [ ] `a.then(b)` (where `A::Out == B::In`) produces a value usable wherever a
      single `ShaderFn` is today (as a channel's encoding function), and the
      generated glue calls `b`'s entry on `a`'s result in one expression, per
      §5.
- [ ] Each link in a chain contributes its own `Params` field to the generated
      `Encodings` struct (not a merged/flattened struct), and the pipeline cache
      signature names every link.
- [ ] Only the chain's first link (the one consuming the stored column) can take
      a per-chunk relative base; a test using a `F32Relative` first link and a
      `Then` second link confirms the base is applied once, correctly.
- [ ] A test chains at least three scales/colour functions
      (`linear.then(sqrt_like).then(sequential)` or an equivalent real
      three-link chain) and checks the GPU result against evaluating the CPU
      mirrors in the same order, within the position/colour tolerance.
- [ ] An external-crate doctest on `gup-core`'s public API builds and renders a
      chart using `.then(..)` on at least one channel.

### AC2: Pow/Sqrt scale

- [ ] A `Pow` scale (with `Sqrt` as `Pow::sqrt()` or equivalent) implements
      `PositionScale` and `CpuMirror`, with a configurable exponent.
- [ ] `conformance`'s GPU≡CPU harness covers `Pow`/`Sqrt` within 0.25 px,
      including near-zero and negative-domain inputs if the scale supports them
      (document the domain restriction if it does not).
- [ ] A golden/structural render (read by eye, and checked through the
      `gup-visual-regression` harness's perceptual tolerance — never a
      byte-exact comparison to a PNG blessed only on this machine) shows a
      scatter with `Circle::RADIUS` driven by a `Sqrt` scale (the RFC's own
      north-star example: size by population) with visibly non-linear radius
      growth and every mark inside the plot rect.

### AC3: Log gains Symlog

- [ ] `Log` supports (or a new `Symlog` type sharing its WGSL module supports) a
      symmetric-log mode that is linear through zero and logarithmic beyond a
      configurable linear threshold, per §5.
- [ ] `conformance` covers `Symlog` within 0.25 px across a domain that
      straddles zero, including points inside and outside the linear threshold.
- [ ] A render with a y-domain that crosses zero (e.g. a signed quantity) using
      `Symlog` shows points on both sides of zero without a gap or a
      discontinuity jump at the threshold, verified by eye.

### AC4: `Time` scale and the precision fix

- [ ] `Time` implements `PositionScale` and `CpuMirror` as "Linear over f64
      seconds," per §5, accepting a numeric (Unix-seconds-like) accessor — this
      story does not require a `chrono`/calendar-type accessor, only
      calendar-aware **ticks**.
- [ ] `Time::ticks` produces calendar-aware tick positions and labels (for
      example, ticks that land on whole days, months or years depending on the
      domain span), not merely evenly spaced numeric ticks.
- [ ] `ColumnStore` (or the `Time` scale's declared input format) caps a chunk's
      value span so that `conformance::chunk_boundary`'s "full chunk of
      one-per-second samples, one-second zoom" scenario, extended with the span
      cap, now passes within 0.25 px (replacing or adding to the existing test
      that documents the miss as a known limit).
- [ ] The existing `conformance::chunk_boundary` tests
      (`relative_chunks_stay_within_a_quarter_pixel_at_the_boundary`,
      `absolute_f32_misses_at_the_boundary`,
      `one_relative_chunk_spanning_years_misses`) still pass unchanged, proving
      the span cap does not regress the already-proven multi-chunk and
      negative-control behaviour.
- [ ] A golden render shows a line or scatter chart with a `Time` x-axis across
      a multi-year domain with calendar tick labels (e.g. year boundaries), read
      by eye through the harness tolerance.
- [ ] A second golden or structural test zooms a `Time`-scaled chart to a
      sub-second window inside a large dataset and confirms (via the
      `Layout`/pick machinery or a direct position check, not merely "looks
      right") that rendered positions match the CPU mirror within 0.25 px at
      that zoom level.

### AC5: Multi-workgroup correctness guardrail

- [ ] Domain fitting for every scale in this story continues to use the
      `ColumnStore`'s existing CPU f64 chunk stats (per RFC-001 §3); no new GPU
      compute reduction is introduced for this story's scales.
- [ ] If the implementation does introduce any GPU compute-shader reduction (for
      example, to accelerate domain fitting at very large row counts), it is
      proven correct when dispatched across more than one workgroup — with a
      dedicated test that forces a multi-workgroup dispatch, not just a
      single-workgroup-sized input — recording explicitly why the old path's
      `compute_basic_stats` was racy and how this test rules out the same class
      of bug. This AC exists as a guardrail regardless of whether this story
      ends up needing it.

### AC6: Browser and old-path freeze

- [ ] `mask wasm-browser` is extended to exercise `then` composition, `Pow`/
      `Sqrt`, `Symlog` and `Time` (at minimum one scene using each), and passes
      on WebGPU/SwiftShader.
- [ ] No file outside `crates/gup-core` (and planning docs) changes;
      `mask old-path-loc` reports the same count as before this story.

## Technical Tasks

- [ ] Generalise the glue emitter's channel source from "one `DynShaderFn`" to
      "a chain of `DynShaderFn`s," threading each link's output as the next
      link's input, emitting one `Encodings` field and one pipeline-signature
      fragment per link, and applying the per-chunk relative base only to the
      first link.
- [ ] Add a `Then<A, B>` (or equivalently named) combinator and wire it through
      `IntoEncoding` so `a.then(b)` type-checks like any other `ShaderFn`-backed
      encoding.
- [ ] Implement `Pow`/`Sqrt` as a `PositionScale` + `CpuMirror`, with its WGSL
      module under `src/shaders`.
- [ ] Extend `Log`'s WGSL/Rust (or add `Symlog`) for the symmetric-log mode.
- [ ] Implement `Time` as a `PositionScale` + `CpuMirror` over f64 seconds, with
      calendar-aware `ticks()`.
- [ ] Extend `ColumnStore`'s chunking (`chunk_rows_for` and the append path)
      with a value-span cap usable by `Time` (and any future scale that declares
      one), opening a new chunk with a fresh origin when the cap would be
      exceeded.
- [ ] Extend `conformance.rs` and `conformance::chunk_boundary` with
      `Pow`/`Sqrt`, `Symlog` and `Time` cases, including the positive,
      span-capped "full chunk" case.
- [ ] Add the `then`-chain GPU≡CPU and pipeline-signature tests (AC1).
- [ ] Add the guardrail test or documentation note for AC5.
- [ ] Extend the `mask wasm-browser` harness scene(s) for AC6.
- [ ] Re-run `mask old-path-loc` and confirm it is unchanged.

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

- [ ] `then` composition, `Pow`/`Sqrt`, `Symlog` and `Time` all pass their
      GPU≡CPU conformance checks within 0.25 px (or 1/255 for colour, where
      applicable).
- [ ] The S4a "full chunk of seconds, one-second zoom" miss is fixed (not merely
      documented) for `Time` columns, proved against
      `conformance::chunk_boundary`.
- [ ] `mask wasm-browser` and `mask old-path-loc` are unaffected beyond the
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

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read) for AC2, AC3
      and AC4
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
