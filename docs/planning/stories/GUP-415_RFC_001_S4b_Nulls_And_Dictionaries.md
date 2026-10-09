# GUP-415: RFC-001 Step S4b: Nulls, Dictionary Encoding and Retain

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-10

## Context

[GUP-414](GUP-414_RFC_001_S4a_Column_Store_Chunking_Append.md) (RFC-001 S4a)
generalises `crates/gup-core/src/column.rs` from one chunk to many, with
per-chunk origins, stats and counted tail append. That story leaves three pieces
of [RFC-001](../rfcs/RFC-001_Core_Architecture.md) §3's design untouched, all
flagged there as S4 scope: nulls, dictionary-encoded categorical columns, and
the `Retain` policy that controls how much CPU data a `Selection` keeps.

- **Nulls.** §3: "Shaders never test for NaN... Position channels with nulls get
  a 1-bit-per-row validity column, and the vertex stage emits a degenerate quad
  for null rows. Colour channels map null to `theme.null_color` through a
  reserved code." Today every column format (`ColumnFormat::F32`/`F32Relative`)
  stores every row's value and there is no concept of "missing".
- **Dictionary encoding.** §3: "Band and categorical scales use `U32` codes.
  Strings and other `Hash + Eq` keys are dictionary-encoded on the CPU, and the
  dictionary order becomes the domain." §12 risk 4, confirmed wrong by S0a
  finding 6, is that a naive `A: Fn(&T) -> D` accessor cannot return a borrow of
  its argument (`|d: &Row| d.continent.as_str()` fails: "lifetime may not live
  long enough"). The fix S0a proved compiles is a dedicated key-accessor bound,
  `for<'a> Fn(&'a T) -> &'a str`, exposed through its own entry point (the RFC
  sketches `color_key`/`encode_key`). Today `encoding.rs`'s `ColumnValue` trait
  is implemented only for `f64`/`f32`; no dictionary, no key accessor, no `U32`
  column format exist.
- **Retain.** §3's `Retain` enum (`Auto`, `Rows`, `Columns`, `GpuOnly`) controls
  whether a `Selection` keeps `rows: Vec<T>` and evaluated CPU columns after
  upload. Today `Selection` always keeps `rows: Vec<T>` unconditionally — there
  is no `GpuOnly` mode, no `Auto` row-count threshold, and no way to opt out of
  CPU retention.

The GUP-406 findings explicitly note S4 is otherwise unaffected by moving shader
composition to build time: a new column format that needs a new entry-point
shape is a glue-emitter change only, and the build step needs nothing. There is
no `Categorical` `ShaderFn` yet — the RFC lists it under S5 ("full scale family
with CPU mirrors"). This story adds the minimum `Categorical` colour mapping
needed to prove the dictionary-encoding path end to end (a `U32` code through a
small Okabe-Ito uniform array to `Color`); S5 is expected to replace or extend
it with the full scale (data-driven domain growth, ticks, legend) once the scale
family lands.

**Input from S4a (GUP-414, 2026-10-10).** RFC-001's "S4a findings" section lists
what this story inherits. The chunk code assumes 4-byte strides: capacities are
64-row blocks, `ColumnStore::chunk_rows_for` divides bytes, and tail writes rely
on `write_buffer`'s 4-byte alignment. A 1-bit validity column therefore needs a
stride in bits, capacities in whole 32-row words, and a tail write that rewrites
the last partial word. Keep the dictionary per store and append-only, so
appended rows never renumber codes. `Retain::GpuOnly` cannot keep `Chunk::bytes`
(the CPU copy that uploads to a second context and the dirty tail use). Drop a
chunk's bytes once it is full and uploaded, and make re-binding to another
context an error in that mode.

## User Story

> "As a visualization developer, I want to encode a categorical column (such as
> a country or species name) as colour, and have missing values render as
> nothing (for position) or a distinct null colour (for colour), so that my
> data's gaps and categories show up correctly without extra plumbing."

## Acceptance Criteria

### AC1: Validity bits for nulls

- [ ] A position channel (X or Y) encoded from an accessor that can produce a
      non-finite value (`f64::NAN`) gets a 1-bit-per-row validity column
      alongside its value column, packed as the RFC specifies.
- [ ] The vertex stage emits a degenerate quad (zero screen-space area, or
      otherwise reliably not rasterised) for rows whose validity bit is unset,
      without the shader testing the value itself for NaN.
- [ ] A test harness (reusing the GUP-388 structural-assertion module) renders a
      selection with some rows' X or Y set to NaN and asserts that no coloured
      pixels appear at those rows' would-be positions, while the other rows
      render normally.

### AC2: Null colour for colour channels

- [ ] A colour channel encoded from an accessor that can produce a null (for a
      dictionary-encoded column, see AC3/AC4) maps null to a reserved code that
      the shader resolves to a fixed null colour, without branching on the input
      value's bit pattern.
- [ ] A test asserts that rows with a null colour-channel value render in the
      null colour (read by eye from the rendered PNG via the harness, not
      inferred indirectly), distinct from every real palette entry.
- [ ] The null colour is a documented constant for this story (RFC-001 S7 has
      not yet built the `Theme` type); the doc comment notes that S7's `Theme`
      is expected to make it configurable and this becomes `theme.null_color`.

### AC3: Dictionary encoding and the key-accessor API

- [ ] A new `ColumnFormat::U32` (or equivalent) stores dictionary codes.
- [ ] A CPU dictionary maps `Hash + Eq` keys (starting with `&str`, per the
      RFC's categorical example) to `u32` codes in first-seen order; that order
      becomes the column's domain.
- [ ] A new entry point on `ShaderFn`/`Selection` accepts an accessor bounded by
      `for<'a> Fn(&'a T) -> &'a str` (addressing RFC §12 risk 4 and S0a finding
      6 directly), so `|d: &Row| d.continent.as_str()` compiles where today's
      `Fn(&T) -> D` accessor cannot.
- [ ] A `trybuild` or doctest proves the naive accessor shape still fails to
      compile with a clear message, and the new entry point succeeds, for the
      same closure shape.

### AC4: Dictionary-encoded colour end to end

- [ ] A minimal `Categorical` colour `ShaderFn` maps a `U32` dictionary code to
      a `Color` through a small uniform array of the Okabe-Ito colour-blind-safe
      palette (the RFC's default for categorical scales).
- [ ] A golden-image test encodes `Circle::FILL` from a categorical string key
      (through the AC3 key-accessor entry point) across several distinct
      categories, and the rendered PNG is read by eye (or compared to a
      checked-in golden) to confirm each category's points carry a distinct,
      correct Okabe-Ito colour.
- [ ] An external-crate doctest (in `gup-core`'s public doc comments, run as
      part of `cargo test --doc`) exercises the same dictionary-colour encoding,
      proving the public API compiles and runs outside the crate.

### AC5: Retain policy

- [ ] A `Retain` enum (`Auto`, `Rows`, `Columns`, `GpuOnly`) is added, with
      `Auto` as `Selection`'s default.
- [ ] `Retain::Auto` keeps `rows: Vec<T>` and evaluated CPU columns while the
      row count is at or below the RFC's 10M-row threshold, and behaves as
      `GpuOnly` above it (frees `rows`/CPU columns after upload, keeping only
      stats and dictionaries).
- [ ] `Retain::GpuOnly` frees `rows`/CPU columns unconditionally after the first
      successful upload; stats and dictionaries remain available (they drive
      domains and are small).
- [ ] A test proves that under `GpuOnly`, `rows: Vec<T>` is actually dropped
      (not merely unused) after `prepare` — for example via a drop counter or
      `Weak` reference on `T`, not just an API-surface check.
- [ ] Re-`attr`-ing a channel, or appending rows (GUP-414), on a `GpuOnly`
      selection documents its limitation clearly (either a clear `Result` error,
      since there is no retained accessor input to re-run, or an explicit
      restriction in the type/method signature) rather than panicking or
      silently doing nothing.

### AC6: Browser

- [ ] `mask wasm-browser` still passes with a scene that exercises the
      dictionary-encoded-colour path (AC4), proving the new entry-point shape
      runs on WebGPU as well as natively.

### AC7: Old path frozen

- [ ] No file outside `crates/gup-core` (and planning docs) changes;
      `mask old-path-loc` reports the same count as before this story.

## Technical Tasks

- [ ] Design and implement the validity-bit column format and its packing
      (alongside the relevant value column, at the chunk level from GUP-414).
- [ ] Update the Circle (and any other mark's) glue emitter to check validity
      for position channels and emit a degenerate quad when unset.
- [ ] Reserve a null code for colour channels and update the relevant glue path
      to resolve it to a fixed null colour without a value-based branch.
- [ ] Add `ColumnFormat::U32` and the CPU dictionary (key → code, first-seen
      order) in `column.rs`/`encoding.rs`.
- [ ] Add the `for<'a> Fn(&'a T) -> &'a str` key-accessor entry point
      (`encode_key`/`color_key` or equivalent) and its `IntoEncoding` impl.
- [ ] Add a `trybuild` case proving the naive `Fn(&T) -> D` shape still fails
      for a borrowing accessor, alongside the new entry point's success case.
- [ ] Add a minimal `Categorical` `ShaderFn` (U32 → Color via a small uniform
      array) with the Okabe-Ito palette as its default resource, documented as a
      placeholder for RFC-001 S5's full scale family.
- [ ] Add the `Retain` enum and wire `Auto`/`GpuOnly` behaviour into `Selection`
      (drop `rows`/CPU columns after upload per policy).
- [ ] Add the drop-proof test for `GpuOnly` retention (AC5).
- [ ] Write the null-position golden/structural test (AC1) and the null-colour
      test (AC2), reusing the GUP-388 harness.
- [ ] Write the dictionary-colour golden test and the external-crate doctest
      (AC4).
- [ ] Extend `mask wasm-browser`'s harness to include a dictionary-encoded
      colour scene (AC6).
- [ ] Re-run `mask old-path-loc` and confirm it is unchanged (AC7).

## Dependencies

### Prerequisite Stories

- GUP-414 (S4a) 📋 — the multi-chunk `ColumnStore` structure (per-chunk origins,
  stats, buffers) that validity bits and dictionary codes are stored alongside.
- GUP-395 ✅ — RFC-001 S0a: `Context`, the typed glue emitter and the Circle
  module this story extends with validity checks and the categorical entry
  point.
- GUP-410 ✅ — GPU error scopes already wrap pipeline creation and the layer
  step of `Plot::resolve`; the new `Categorical` pipeline and glue shape must
  keep running inside that scope (soft — no API change expected).

### Enables Stories

- RFC-001 S5 (full scale family, not yet written as a story) — replaces this
  story's minimal `Categorical` `ShaderFn` with the full scale (data-driven
  domain growth, ticks, legend), reusing the `U32`-dictionary column format and
  key-accessor entry point this story builds.
- RFC-001 S7 (`Theme`, not yet written as a story) — wires this story's fixed
  null colour constant into `theme.null_color`.

## Testing Strategy

- **Unit tests**: validity-bit packing; dictionary key-to-code assignment and
  domain ordering; `Retain::Auto`'s threshold behaviour; the `GpuOnly`
  drop-proof test.
- **Integration tests**: the null-position render (AC1), the null-colour render
  (AC2), and the dictionary-encoded-colour golden (AC4), all read by eye by the
  implementer, not inferred from pixel counts alone.
- **Compile-time tests**: a `trybuild` pair showing the naive borrowing accessor
  still fails and the new key-accessor entry point succeeds (AC3).
- **Doctest**: an external-crate-style doctest exercising the dictionary-color
  path end to end (AC4).
- **Browser**: `mask wasm-browser` extended to cover the dictionary-colour scene
  (AC6).

## Success Metrics

- [ ] Null position values never rasterise, and null colour values render in a
      distinct, documented null colour — both verified by eye.
- [ ] `|d: &Row| d.continent.as_str()` (or equivalent) compiles through the new
      key-accessor entry point and renders the correct Okabe-Ito colour per
      category.
- [ ] `Retain::GpuOnly` actually frees `rows: Vec<T>` after upload, proved by a
      drop/weak-reference test, not just documentation.
- [ ] `mask wasm-browser` and `mask old-path-loc` are unaffected.

## Risk Assessment

- **Medium**: the minimal `Categorical` `ShaderFn` built here may not match what
  RFC-001 S5's full scale family needs (domain growth, legend, ticks), risking
  throwaway work. _Mitigation_: keep it deliberately small (just the
  `U32`-to-`Color` uniform-array lookup) and document it explicitly as a
  stand-in S5 is expected to replace, matching how S0a's minimal
  Linear/Log/Sequential were later folded into S5's full family without wasted
  column-format or glue-emitter work.
- **Medium**: validity bits interacting with per-chunk relative origins (a null
  value has no meaningful position to be "relative to") needs a clear rule — for
  example, a null row's value column entry is never read because the vertex
  stage returns early, so its stored bytes (and its effect, if any, on the
  chunk's origin/stats) must be well-defined and tested. _Mitigation_: exclude
  null rows from `ColumnStats` (`non_finite` already exists for this) and
  document that a null row's value-column bytes are unspecified.
- **Low**: adding a validity column consumes one of the 8 vertex-buffer slots
  (RFC-001 §12 risk 7) per position channel that allows nulls. _Mitigation_:
  pack validity as a shared bitmask column (one extra column per chunk, not per
  nullable channel) if more than one channel needs it; document the 8-slot
  ceiling's interaction with this story's validity column in code comments for
  S6 (`derive(Mark)`) to enforce formally.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read) for AC1, AC2
      and AC4
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
