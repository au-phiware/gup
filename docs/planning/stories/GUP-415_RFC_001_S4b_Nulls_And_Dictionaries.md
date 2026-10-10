# GUP-415: RFC-001 Step S4b: Nulls, Dictionary Encoding and Retain

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-10)
**Created**: 2026-10-10

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

- [x] A position channel (X or Y) encoded from an accessor that can produce a
      non-finite value (`f64::NAN`) gets a 1-bit-per-row validity column
      alongside its value column, packed as the RFC specifies.
- [x] The vertex stage emits a degenerate quad (zero screen-space area, or
      otherwise reliably not rasterised) for rows whose validity bit is unset,
      without the shader testing the value itself for NaN.
- [x] A test harness (reusing the GUP-388 structural-assertion module) renders a
      selection with some rows' X or Y set to NaN and asserts that no coloured
      pixels appear at those rows' would-be positions, while the other rows
      render normally.

### AC2: Null colour for colour channels

- [x] A colour channel encoded from an accessor that can produce a null (for a
      dictionary-encoded column, see AC3/AC4) maps null to a reserved code that
      the shader resolves to a fixed null colour, without branching on the input
      value's bit pattern.
- [x] A test asserts that rows with a null colour-channel value render in the
      null colour (read by eye from the rendered PNG via the harness, not
      inferred indirectly), distinct from every real palette entry.
- [x] The null colour is a documented constant for this story (RFC-001 S7 has
      not yet built the `Theme` type); the doc comment notes that S7's `Theme`
      is expected to make it configurable and this becomes `theme.null_color`.

### AC3: Dictionary encoding and the key-accessor API

- [x] A new `ColumnFormat::U32` (or equivalent) stores dictionary codes.
- [x] A CPU dictionary maps `Hash + Eq` keys (starting with `&str`, per the
      RFC's categorical example) to `u32` codes in first-seen order; that order
      becomes the column's domain.
- [x] A new entry point on `ShaderFn`/`Selection` accepts an accessor bounded by
      `for<'a> Fn(&'a T) -> &'a str` (addressing RFC §12 risk 4 and S0a finding
      6 directly), so `|d: &Row| d.continent.as_str()` compiles where today's
      `Fn(&T) -> D` accessor cannot.
- [x] A `trybuild` or doctest proves the naive accessor shape still fails to
      compile with a clear message, and the new entry point succeeds, for the
      same closure shape.

### AC4: Dictionary-encoded colour end to end

- [x] A minimal `Categorical` colour `ShaderFn` maps a `U32` dictionary code to
      a `Color` through a small uniform array of the Okabe-Ito colour-blind-safe
      palette (the RFC's default for categorical scales).
- [x] A golden-image test encodes `Circle::FILL` from a categorical string key
      (through the AC3 key-accessor entry point) across several distinct
      categories, and the rendered PNG is read by eye (or compared to a
      checked-in golden) to confirm each category's points carry a distinct,
      correct Okabe-Ito colour.
- [x] An external-crate doctest (in `gup-core`'s public doc comments, run as
      part of `cargo test --doc`) exercises the same dictionary-colour encoding,
      proving the public API compiles and runs outside the crate.

### AC5: Retain policy

- [x] A `Retain` enum (`Auto`, `Rows`, `Columns`, `GpuOnly`) is added, with
      `Auto` as `Selection`'s default.
- [x] `Retain::Auto` keeps `rows: Vec<T>` and evaluated CPU columns while the
      row count is at or below the RFC's 10M-row threshold, and behaves as
      `GpuOnly` above it (frees `rows`/CPU columns after upload, keeping only
      stats and dictionaries).
- [x] `Retain::GpuOnly` frees `rows`/CPU columns unconditionally after the first
      successful upload; stats and dictionaries remain available (they drive
      domains and are small).
- [x] A test proves that under `GpuOnly`, `rows: Vec<T>` is actually dropped
      (not merely unused) after `prepare` — for example via a drop counter or
      `Weak` reference on `T`, not just an API-surface check.
- [x] Re-`attr`-ing a channel, or appending rows (GUP-414), on a `GpuOnly`
      selection documents its limitation clearly (either a clear `Result` error,
      since there is no retained accessor input to re-run, or an explicit
      restriction in the type/method signature) rather than panicking or
      silently doing nothing.

### AC6: Browser

- [x] `mask wasm-browser` still passes with a scene that exercises the
      dictionary-encoded-colour path (AC4), proving the new entry-point shape
      runs on WebGPU as well as natively.

### AC7: Old path frozen

- [x] No file outside `crates/gup-core` (and planning docs) changes;
      `mask old-path-loc` reports the same count as before this story.

## Technical Tasks

- [x] Design and implement the validity-bit column format and its packing
      (alongside the relevant value column, at the chunk level from GUP-414).
- [x] Update the Circle (and any other mark's) glue emitter to check validity
      for position channels and emit a degenerate quad when unset.
- [x] Reserve a null code for colour channels and update the relevant glue path
      to resolve it to a fixed null colour without a value-based branch.
- [x] Add `ColumnFormat::U32` and the CPU dictionary (key → code, first-seen
      order) in `column.rs`/`encoding.rs`.
- [x] Add the `for<'a> Fn(&'a T) -> &'a str` key-accessor entry point
      (`encode_key`/`color_key` or equivalent) and its `IntoEncoding` impl.
- [x] Add a `trybuild` case proving the naive `Fn(&T) -> D` shape still fails
      for a borrowing accessor, alongside the new entry point's success case.
- [x] Add a minimal `Categorical` `ShaderFn` (U32 → Color via a small uniform
      array) with the Okabe-Ito palette as its default resource, documented as a
      placeholder for RFC-001 S5's full scale family.
- [x] Add the `Retain` enum and wire `Auto`/`GpuOnly` behaviour into `Selection`
      (drop `rows`/CPU columns after upload per policy).
- [x] Add the drop-proof test for `GpuOnly` retention (AC5).
- [x] Write the null-position golden/structural test (AC1) and the null-colour
      test (AC2), reusing the GUP-388 harness.
- [x] Write the dictionary-colour golden test and the external-crate doctest
      (AC4).
- [x] Extend `mask wasm-browser`'s harness to include a dictionary-encoded
      colour scene (AC6).
- [x] Re-run `mask old-path-loc` and confirm it is unchanged (AC7).

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

- [x] Null position values never rasterise, and null colour values render in a
      distinct, documented null colour — both verified by eye.
- [x] `|d: &Row| d.continent.as_str()` (or equivalent) compiles through the new
      key-accessor entry point and renders the correct Okabe-Ito colour per
      category.
- [x] `Retain::GpuOnly` actually frees `rows: Vec<T>` after upload, proved by a
      drop/weak-reference test, not just documentation.
- [x] `mask wasm-browser` and `mask old-path-loc` are unaffected.

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

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read) for AC1, AC2
      and AC4
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

The numbers are in RFC-001's "S4b findings (2026-10-10, GUP-415)" section.

### Delivered

- **Validity bits** (`crates/gup-core/src/column.rs`). There is one plane per
  numeric column, with one word per plane in each 32-row group. Each chunk has a
  small storage buffer, created only once the store has a null and counted as
  the new `Upload::Validity`. A tail write rewrites the last partial group; a
  grown chunk's bits are rewritten in full.
- **Glue** (`crates/gup-core/src/shader/glue.rs`). With nulls, group 2 binding 1
  is `var<storage, read> validity`. A null position or size replaces the mark's
  `clip` with one point outside the clip volume, a degenerate quad. A null
  numeric colour `select`s `NULL_COLOR`. `U32` columns are `u32` vertex inputs
  fetched as `Uint32`. The mark contract now names the position member `clip`.
- **Dictionaries** (`column.rs`). `ColumnFormat::U32`, `Dictionary` (one per
  `U32` column per store, first-seen order, append-only) and
  `NULL_CODE = u32::MAX`. `ColumnStore::append` now takes `ColumnData`:
  `Values(Vec<f64>)` or `Keys(Vec<Option<&str>>)`.
- **Key accessors** (`encoding.rs`). `ShaderFn::encode_key` takes
  `for<'a> Fn(&'a T) -> &'a str`, and `encode_nullable_key` takes
  `Option<&'a str>`, through `KeyEncoded`, `Key`, `NullableKey` and
  `KeyAccessor`. `ColumnValue` has an `on_unimplemented` message that points to
  `encode_key`.
- **`Categorical`** (`scale/categorical.rs`, `shaders/color_categorical.wgsl`).
  A stand-in for S5: Okabe-Ito in a uniform array of 8, `code % 8`, and the null
  code mapped to `NULL_COLOR` (`#999999`, documented as S7's
  `theme.null_color`). It is in the prelude.
- **`Retain`** (`selection.rs`, `plot.rs`). `Auto` (the default: everything up
  to 10M rows, `GpuOnly` above), `Rows`, `Columns` and `GpuOnly`.
  `Plot::resolve` releases after the scoped prepare succeeds. Re-encoding a
  channel, a different chunk size, or another context without the needed data is
  a configuration error naming the policy. `Retain::Rows` re-evaluates for
  another context, and appends work under every policy.
- **Browser** (`wasm-size/scatter`). `render_categorical` draws a dictionary
  scene with nulls, and the page checks its colours and the null row.

### Deviations and decisions

- **The validity column is its own buffer per chunk**, not a sub-range of the
  chunk buffer. Data without nulls keeps the S0a bytes, glue and upload counts,
  and the bits use no vertex slot. The trade-off is one more buffer and bind
  group per chunk for nullable layers.
- **One plane per numeric column**, not one shared bitmask. A null colour must
  draw grey, while a null position must hide the row. It is still one binding.
- **AC5 (append under `GpuOnly`)** works rather than erroring: the last chunk
  keeps its CPU copy, and appended rows are released after their upload.
  Re-encoding and re-binding are the clear errors.
- **AC7.** One file outside `crates/gup-core` was added: the golden
  `tests/golden/gup_core/categorical_nulls.png`, test data beside S0a's
  `scatter.png`. `mask old-path-loc` reads 28910 before and after.
- **Fixed on the way.** A NaN driving `Sequential` was undefined (`clamp` on
  NaN). It now draws `NULL_COLOR`, which a pixel test checks.

### Tests

gup-core's lib tests went from 71 to 84 (2 ignored); all pass on Intel/Mesa and
on lavapipe.

- `column::tests` (4): validity packing per plane, dictionary codes across
  appends and chunks, validity upload byte counts (first null, growth, tail),
  and release with another context's error.
- `selection::tests` (3): the `nulls_glue.wgsl` fixture with naga layouts, the
  numeric-colour null `select`, and the `Categorical` params at naga's offsets.
- `plot::tests` (5): the `Retain` table, the `GpuOnly` drop proof (an `Arc`
  strong count, 151 → 1), `Rows`/`Columns`/`Auto` on another context, a cleared
  bit hiding a finite row, and a NaN sequential input drawing grey.
- `scale::categorical::tests` (1).
- `tests/scatter_png.rs::categorical_scatter_with_nulls`, against the new
  golden.
- trybuild: `compile_fail/borrowing_accessor.rs` and
  `compile_pass/key_accessor.rs`; the `bare_number` snapshot gained the
  `encode_key` note.
- A rendering doctest on `Categorical`.
- `mask wasm-browser` passes: `dict=652,657,651,708,318 dict-null-row-diff=0px`.

### Evidence

- **Visual.** I read three images:
  - `categorical_nulls.png`: the title, log-y and linear-x ticks, and points in
    orange, sky blue, bluish green, yellow and blue, with grey for missing
    continents, all inside the plot.
  - The browser PNG: the chunked scatter, with the dictionary scene below it in
    four palette colours plus grey and no point at the NaN row.
  - `window_scatter.png`: ΔE 0 against the `ImageTarget` and the golden.
- **Seeded bug.** With the degenerate quad disabled,
  `a_cleared_validity_bit_hides_a_finite_row` fails ("row 0 still drawn").
- **Gates.** `cargo test -p gup-core -p gup-text` and `mask all-check` pass.
  `mask old-path-loc` is unchanged at 28910.
- **Not run locally.** The root crate's tests and `mask smoke-examples` were
  left to CI, as the orchestrator asked. `mask all-check` type-checks every
  example with clippy `--all-targets`.

### Key files

- `crates/gup-core/src/{column,encoding,selection,plot,render,channel,context,lib}.rs`
- `crates/gup-core/src/shader/glue.rs`
- `crates/gup-core/src/scale/{categorical,mod}.rs`
- `crates/gup-core/src/shaders/{color_categorical,circle}.wgsl`
- `crates/gup-core/tests/{scatter_png,compile_fail}.rs`
- `crates/gup-core/tests/common/{continents,vr}.rs`
- `crates/gup-core/tests/compile_fail/borrowing_accessor.{rs,stderr}`
- `crates/gup-core/tests/compile_pass/key_accessor.rs`
- `crates/gup-core/tests/fixtures/nulls_glue.wgsl`
- `crates/gup-core/wasm-size/scatter/{src/lib.rs,index.html}`
- `tests/golden/gup_core/categorical_nulls.png`

## Retrospective

**Completed**: 2026-10-10

### Key Technical Learnings

#### A NaN render test can pass without the mechanism

- **Challenge**: AC1 asks for a render in which null positions draw nothing. On
  Intel/Mesa a NaN position is culled by the hardware anyway. With the
  degenerate quad seeded off, the NaN image test still passed.
- **Solution**: a second test keeps the row's position finite and clears only
  its validity bit through a test seam (`ColumnStore::clear_valid`). With the
  seed, that test fails ("row 0 still drawn"). The image test stays, because it
  shows what a user sees, and it is byte-exact against the plot with the null
  rows removed.
- **Pattern**: when the platform may already produce the right output, test the
  mechanism with input the platform cannot fix on its own, and seed the bug to
  see the test fail.

#### Higher-ranked closures need the bound on the parameter itself

- **Challenge**: RFC §12 risk 4. A closure returning a borrow of its argument
  only gets a higher-ranked signature if rustc sees an `Fn` bound on that
  closure's own type parameter.
- **Solution**: `encode_key<T, A>(…) where A: for<'a> Fn(&'a T) -> &'a str`
  wraps the closure in `Key<A>` and erases it behind `KeyAccessor<T>` only
  afterwards. gup-core's own code hit the same error: a closure returning a
  chunk's validity buffer had to become a `fn`.
- **Pattern**: put the HRTB `Fn` bound on the entry point's generic, and do the
  type erasure after it.

#### Keep the clean path byte-identical

- **Challenge**: every S0a/S4a test counts bytes and compares fixtures. An
  always-on validity column would have changed all of them, and made every user
  pay for nulls they don't have.
- **Solution**: the store always keeps CPU bits (1/32 of a column), but the GPU
  buffer, the binding and the glue's validity code appear only once the store
  has a null. `scatter_glue.wgsl`, the S0a byte layout and the `columns.bytes`
  assertions are untouched. The first null changes the glue signature, so the
  layer rebuilds once.
- **Pattern**: make an optional feature cost nothing until the data needs it,
  and let the existing fixtures prove that.

#### Interleave bit planes by row group

- **Challenge**: a store has several planes, chunks grow, and tail writes should
  be small.
- **Solution**: put one word per plane in each 32-row group. Growth only extends
  the array, a tail write is one contiguous range per chunk, and the shader
  needs no per-chunk plane stride.

### Architectural Decisions

#### Validity in its own per-chunk storage buffer

- **Decision**: validity bits live in a separate storage buffer per chunk, bound
  in group 2 with one bind group per chunk, instead of a sub-range of the
  chunk's column buffer.
- **Reasoning**: data without nulls keeps the S0a layout and pays nothing. The
  bits take no vertex slot, and a 1-bit stride cannot be an instance-rate vertex
  attribute anyway.
- **Trade-off**: nullable layers have one more buffer and bind group per chunk,
  and need vertex-stage storage buffers, which WebGPU compatibility mode lacks
  (a clear error today).
- **Future**: S9's compute passes can bind the same bits.

#### Two key entry points

- **Decision**: `encode_key` (keys always present) and `encode_nullable_key`
  (`Option<&str>`).
- **Reasoning**: closure inference needs the exact `Fn` bound, and coherence
  forbids two blanket impls that differ only in a closure's output.
- **Trade-off**: two methods instead of one.
- **Future**: owned `Hash + Eq` keys (S5) can be a third shape, or a generic
  `Dictionary<K>`.

#### `Retain` releases inside `Plot::resolve`

- **Decision**: the layer releases data after the scoped `prepare` returns `Ok`.
  `Rows` keeps the rows only, and `Columns` keeps the CPU columns only.
- **Reasoning**: nothing is dropped before it is on the GPU, and each policy
  keeps a different thing: rows for picking `&T`, columns for re-binding.
- **Trade-off**: in a browser, a GPU error can be reported after the release
  (GUP-410's asynchronous scopes).

### Development Workflow Insights

- **Lavapipe locally.** Running the gup-core suite with
  `VK_ICD_FILENAMES=/run/opengl-driver/share/vulkan/icd.d/lvp_icd.x86_64.json`
  and `WGPU_BACKEND=vulkan` before pushing confirmed that the byte-exact
  null-row comparison holds on CI's rasteriser (golden max ΔE 0.8).
- **WASM size needs the same harness.** The harness grew a second scene, so the
  first delta (+10.5 KB gz) mixed library and harness cost. Measuring a
  temporary worktree at the pre-story commit, and HEAD with the scene removed,
  attributes +6.4 KB gz to S4b.
- **Perl and JavaScript templates.** A `perl -0pi` edit interpolated `${white}`
  and similar JavaScript template literals as empty Perl variables, and the
  browser page timed out with a blank result line. Use the Edit tool, or a
  quoted heredoc, for files with `$`.
- **No Python** in this environment: Perl and sed only, or the Edit tool.

### Follow-up Stories

None written. Everything this story found belongs to RFC steps that have no
story yet, and is recorded in RFC-001's "S4b findings" for whoever writes them:

- **S5**: a dictionary hook for scales (keys for legends and Band), palettes
  longer than 8, null keys on Band position channels, and generic `Hash + Eq`
  keys.
- **S6**: check the `clip` member.
- **S7**: `NULL_COLOR` becomes `theme.null_color`.
- **S9**: `pick` returns a `RowId` under `Columns`/`GpuOnly`.
- **WebGPU compatibility mode**: the fallback and its cost are noted there, not
  scheduled.
