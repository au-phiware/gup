# GUP-419: RFC-001 Step S5b: Colour Scales, the Dictionary Hook, Band and Point

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-11)
**Created**: 2026-10-10

## Context

[RFC-001](../rfcs/RFC-001_Core_Architecture.md) §5 lists the scale family as
"Linear, Pow/Sqrt, Log (with Symlog), Time..., Band, Point, Sequential
(256-entry LUT texture), Diverging and Categorical (uniform `array<vec4,16>`,
Okabe-Ito by default)."
[GUP-418](GUP-418_RFC_001_S5a_ShaderFn_V2_And_Numeric_Scales.md) (S5b's sibling,
S5a) covers the numeric position scales that need no dictionary (Linear,
Pow/Sqrt, Log/Symlog, Time) and the `then` composition primitive. This story
completes the family: the colour scales (`Sequential` already exists from S0a;
`Diverging` and the full `Categorical` are new), the ordinal position scales
that need dictionary keys (`Band`, `Point`), and the **dictionary hook** that
all of them share.

**Why a hook, not just a bigger `Categorical`.**
[GUP-415](GUP-415_RFC_001_S4b_Nulls_And_Dictionaries.md) (S4b) built
dictionary-encoded `U32` columns, `ShaderFn::encode_key`/`encode_nullable_key`,
and a deliberately minimal `Categorical` ("a stand-in... It ignores the domain;
codes cycle past 8," its own doc comment calling it "a placeholder for RFC-001
S5's full scale family"). S4b's findings record exactly what is missing for the
full family:

> "Replace `Categorical` with the full scale. It needs the dictionary, not
> `fit_domain`'s numeric extent: add a hook that hands the layer's `Dictionary`
> to the function (keys for legends and Band positions, `len()` for the domain).
> A new key is then a domain change (a uniform write), with no column bytes.
> Palettes longer than 8 need a length-generic uniform or a LUT texture."
>
> "`Band` on X/Y: a `U32` column has no validity plane, so a null key on a
> position channel is not hidden today. Either give geometry `U32` columns a
> plane or have the glue compare `NULL_CODE` (a reserved code, not a NaN test)."
>
> "Other keys: `Dictionary` holds strings. Integer and enum keys (`Hash + Eq`)
> need owned keys and a `Display` for labels: a generic `Dictionary<K>` behind
> the same `U32` format."

Today's `ColumnStore::Dictionary` is `&str`-keyed only, and no `ShaderFn` has
access to it: `fit_domain(&mut self, extent: (f64, f64))` only ever receives a
numeric min/max, which a dictionary-driven scale cannot use (its domain is "how
many distinct keys," not a numeric range). `Categorical`'s uniform array is
fixed at 8 entries and does not grow with the dictionary. No `Band` or `Point`
scale exists.

## User Story

> "As a visualization developer, I want a categorical colour scale whose legend
> and palette size follow my data's actual categories (not a fixed stand-in),
> and band/point position scales for categorical axes, with missing categories
> hidden or shown in a clear null colour, so that bar charts, grouped scatter
> plots and their legends are correct without me managing a dictionary by hand."

## Acceptance Criteria

### AC1: The dictionary hook

- [x] A `ShaderFn` that needs dictionary-driven domain information (key count,
      and the keys themselves in first-seen order for legend labels) receives
      them through a dedicated hook, analogous to how `fit_domain` already
      receives a numeric extent — this story's hook replaces or extends
      `fit_domain` for dictionary-backed channels; document which.
- [x] A new dictionary key on a resolved chart's data changes only a uniform
      (the domain grows), with no column bytes rewritten for rows that already
      had codes — proved by the existing upload-counting infrastructure
      (`Context::write_buffer`/`Upload` kinds from S0b/S4a), not merely
      asserted.
- [x] A test exercises the hook end to end: a `Categorical`-encoded column grows
      its dictionary (via append or a second resolve with new rows), and the
      scale's domain/legend reflect the new key without a column rewrite.

### AC2: Full `Categorical` colour scale

- [x] `Categorical` maps dictionary codes to colours from the Okabe-Ito palette
      (RFC default), now sized to the dictionary's actual key count, not a fixed
      cycle of 8.
- [x] Palettes longer than 8 are supported: the implementation picks a
      length-generic uniform array or a LUT texture (reusing the `Resource`
      mechanism `Sequential` already proves) and documents the choice and its
      size limit, if any.
- [x] `Categorical::legend()` (per `ColorScale`'s trait method, RFC §5) returns
      labelled swatches in the dictionary's first-seen order, each swatch's
      colour matching what the GPU renders for that key — checked by a test that
      renders a legend and the data with the same scale and compares colours.
- [x] A golden render (read by eye, and checked through the
      `gup-visual-regression` harness's perceptual tolerance — never a
      byte-exact comparison to a PNG blessed only on this machine) shows a
      scatter chart with more than 8 distinct categories, each with a visibly
      distinct colour, and a legend whose swatches and labels match.

### AC3: `Diverging` colour scale

- [x] `Diverging` implements `ColorScale` + `CpuMirror`: two colour ramps
      meeting at a configurable midpoint domain value, via a LUT texture (the
      same mechanism as `Sequential`).
- [x] `conformance` covers `Diverging` within 1/255 per channel, including
      values at, above and below the midpoint.
- [x] A golden render shows a chart with a signed quantity (e.g. temperature
      anomaly) coloured by `Diverging`, with visually distinct colours on each
      side of the midpoint and a legend gradient bar matching the data colours
      at both extremes (reusing the `ItemKind::Gradient` scene item from S3).

### AC4: `Band` and `Point` position scales

- [x] `Band` implements `PositionScale` (`In = u32`, `Out = Px`) over dictionary
      codes, with configurable padding (inner/outer) per category, and a
      `band_width()` (or equivalent) accessor so a mark (for example, a future
      bar mark) can size itself to the band.
- [x] `Point` implements `PositionScale` over dictionary codes, placing each
      category at the centre of its slot with no width (for scatter-style
      categorical axes).
- [x] A null key on a `Band`/`Point` X or Y channel is hidden (not drawn), using
      whichever mechanism the implementer chooses (a validity plane on `U32`
      geometry columns, or a `NULL_CODE` comparison in the glue) — document the
      choice and why.
- [x] `conformance` covers `Band`/`Point` positions against their CPU mirrors
      within 0.25 px across a dictionary of several categories, including a
      null-key row proven hidden (not merely "renders at some position").
- [x] A golden render shows a categorical x-axis (`Band` or `Point`) with tick
      labels drawn from the dictionary's keys (not numeric codes), read by eye
      through the harness tolerance.

### AC5: Generic `Dictionary<K>`

- [x] A generic `Dictionary<K>` (or equivalent) supports `Hash + Eq + Display`
      keys beyond `&str` (for example, integers or an enum), behind the same
      `U32` column format, with its own key-accessor entry point(s) analogous to
      `encode_key`/`encode_nullable_key`.
- [x] A test (unit or doctest) drives `Categorical` or `Band` from a non- string
      key type and confirms correct domain ordering and label formatting (via
      `Display`).

### AC6: Browser and old-path freeze

- [x] `mask wasm-browser` is extended to exercise `Diverging`, the full
      `Categorical` (with a palette larger than 8, and its legend), and
      `Band`/`Point` (including a hidden null key), and passes on
      WebGPU/SwiftShader.
- [x] No file outside `crates/gup-core` (and planning docs) changes;
      `mask old-path-loc` reports the same count as before this story.

## Technical Tasks

- [x] Design and implement the dictionary hook on `ShaderFn` (or a sibling
      trait), replacing/extending `fit_domain` for dictionary-backed channels,
      and wire it through `Plot::resolve`.
- [x] Rebuild `Categorical` on the hook: dictionary-sized palette (uniform array
      or LUT, decide and document), `legend()`, and its WGSL module.
- [x] Implement `Diverging` (WGSL module, CPU mirror, LUT-backed) and its
      `legend()`.
- [x] Implement `Band` and `Point` (WGSL modules, CPU mirrors) over dictionary
      codes, including padding/width for `Band`.
- [x] Resolve the null-key-on-position question (validity plane vs. `NULL_CODE`
      compare) for `U32` geometry columns and implement it in the glue emitter.
- [x] Add a generic `Dictionary<K>` and its key-accessor entry points.
- [x] Extend `conformance.rs` with `Diverging`, `Categorical` (dictionary-
      sized), `Band` and `Point` cases, including the null-key case.
- [x] Add the dictionary-hook end-to-end test (AC1) and the legend-matches-
      data-colour test (AC2/AC3).
- [x] Extend the `mask wasm-browser` harness scene(s) for AC6.
- [x] Re-run `mask old-path-loc` and confirm it is unchanged.

## Dependencies

### Prerequisite Stories

- GUP-415 (S4b) ✅ — the `U32` dictionary column format, `encode_key`/
  `encode_nullable_key`, and the minimal `Categorical` this story replaces.
- GUP-414 (S4a) ✅ — the multi-chunk `ColumnStore`, whose per-chunk stats and
  dictionary this story's hook reads.
- GUP-418 (S5a) 📋 (soft) — shares the `ShaderFn` v2 shape and the glue
  emitter's generalised channel-source handling; this story's scales do not need
  `then` composition directly, but should follow the same trait and signature
  conventions S5a settles.
- GUP-401 (S3) ✅ — the `ItemKind::Gradient` scene item this story's `Diverging`
  legend reuses.

### Enables Stories

- RFC-001 S6 (`derive(Mark)`, marks beyond Circle; not yet written) — needs to
  check the `clip` position member against `Band`/`Point`'s null-key handling,
  per S4b's findings.
- RFC-001 S7 (`Theme`, guide emitters; not yet written) — `NULL_COLOR` becomes
  `theme.null_color`; axis guides for `Band`/`Point` need dictionary-label tick
  rendering.
- RFC-001 S10 (porting `bar` onto `Plot`; not yet written) — a bar chart builder
  needs `Band`'s width accessor directly.

## Testing Strategy

- **Unit tests**: dictionary hook domain/key propagation; `Band`/`Point`
  position formulas; generic `Dictionary<K>` ordering and `Display` formatting.
- **Conformance tests**: GPU≡CPU within 0.25 px (position) or 1/255 (colour) for
  `Diverging`, the full `Categorical`, `Band` and `Point`, including a null-key
  case.
- **Golden/visual tests**: read by eye through the `gup-visual-regression`
  harness's perceptual tolerance (never byte-exact against a PNG blessed only on
  the implementer's machine) for AC2, AC3 and AC4.
- **Doctest**: an external-crate-style doctest for AC5 (a non-string dictionary
  key).
- **Browser**: `mask wasm-browser` extended per AC6.

## Success Metrics

- [x] A new dictionary key grows a scale's domain with 0 column bytes rewritten,
      measured through the existing upload counters.
- [x] `Categorical`'s legend colours match the GPU-rendered data colours for
      every category, including palettes larger than 8.
- [x] A null key on a `Band`/`Point` position channel is proven hidden, not
      merely assumed from the render.
- [x] `mask wasm-browser` and `mask old-path-loc` are unaffected beyond the
      intended scene additions.

## Risk Assessment

- **Medium**: the dictionary hook is new plumbing through `Plot::resolve` that
  every dictionary-backed scale depends on; getting its shape wrong risks a
  second round of churn once S6/S7 land. _Mitigation_: keep the hook minimal
  (keys in order, plus `len()`), matching exactly what S4b's findings specify is
  needed, and defer anything speculative (per-key styling, for example) to a
  later story.
- **Medium**: choosing between a length-generic uniform array and a LUT texture
  for `Categorical` affects the WGSL module's shape and is hard to change later
  without a second pipeline-signature bump. _Mitigation_: prefer reusing the
  `Resource::Lut` mechanism `Sequential` already proves (nearest-sample lookup
  by code, not linear filtering) unless a concrete reason favours a uniform
  array; record the decision and its trade-off in the story's Implementation
  Summary.
- **Low**: the null-key-on-position choice (validity plane vs. `NULL_CODE`
  compare) affects the 8 vertex-buffer-slot budget (RFC-001 §12 risk 7) that
  S6's `derive(Mark)` will enforce formally. _Mitigation_: prefer the
  `NULL_CODE` compare (no extra buffer or slot) unless a concrete case needs the
  validity plane's generality; document the choice for S6.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read) for AC2, AC3
      and AC4
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

All in `crates/gup-core`, plus four goldens under `tests/golden/gup_core/`
(three new, one re-blessed) and RFC-001's "S5b findings". `mask old-path-loc`:
28,910, unchanged.

- **Dictionary hook** (`src/encoding.rs`, `src/column.rs`, `src/selection.rs`):
  - `ShaderFn::fit_keys(&mut self, keys: &[Arc<str>])` **replaces** `fit_domain`
    for dictionary channels: a key channel's first link gets the keys (labels in
    first-seen order, `len()` the domain), never a numeric extent.
    `EncodeFn::fit_dictionary` forwards it; `Then` fits later links to the image
    of codes `0..len`.
  - Dictionaries move from the column store into each key channel's encoding as
    a generic `Dictionary<K>` (`K: DictionaryKey`, i.e.
    `Hash + Eq + Clone + Display`). The store holds codes (`ColumnData::Codes`).
  - New entry points: `encode_owned_key` and `encode_nullable_owned_key` for
    integer, enum and other owned keys, and `KeyEncoded::domain(keys)` to fix
    the domain's order (calendar months, stable colours).
- **`Categorical`** (`src/scale/categorical.rs`,
  `src/shaders/color_categorical.wgsl`):
  - Choice: a **LUT texture read by texel** (`textureLoad`), reusing
    `Resource::Lut`, with a uniform key count. Size limit: palettes of up to
    2,048 colours (`MAX_PALETTE`). A uniform array would need its length fixed
    in WGSL and would grow the `Encodings` uniform.
  - Codes outside the domain, the null code among them, draw `NULL_COLOR`.
  - Okabe-Ito is extended past 8 keys to 64 by greedy farthest-in-OKLab colours,
    stable as the domain grows. Past 64, colours repeat.
  - `ColorScale::legend()` returns `Legend::Swatches` (label and colour, in
    domain order).
- **Legends** (`src/scale/legend.rs`, `src/scene.rs`): the `ColorScale` trait
  and `Legend`, `Swatch`, `Ramp` and `RampTick`. `Sequential::ramp()` and
  `Diverging::ramp()` give a `Ramp`, which `GradientBar::new` draws (replacing
  `GradientBar::sequential`).
- **`Diverging`** (`src/scale/diverging.rs`,
  `src/shaders/color_diverging.wgsl`): two slopes meeting at a midpoint, over a
  257-entry blue–white–red LUT. The automatic domain is symmetric about the
  midpoint, and `domain(d0, mid, d1)` sets it explicitly.
- **`Band` and `Point`** (`src/scale/band.rs`, `src/shaders/scale_band.wgsl`):
  - One function, `offset + step · code`, mapping each key to its slot's centre.
  - Band has inner and outer padding plus `band_width()` and `step()`; Point has
    outer padding.
  - Each key is a tick, labelled with the key. `PositionScale` no longer
    requires an `f32` input.
- **Null key on a position** (`src/shader/glue.rs`): chosen the **`NULL_CODE`
  compare**. For a `U32` column on a non-colour channel, the glue ANDs
  `u32(col.<ch> != 4294967295u)` into `drawn`. No validity plane, storage read
  or vertex slot is needed (§12 risk 7), because codes already reserve a null.
- **Tick labels** use U+2212 MINUS SIGN (`src/scale/mod.rs`, `log.rs`,
  `symlog.rs`).
- **Tests**:
  - Conformance (Intel / lavapipe):
    - `Diverging`: 2.97e-5 / 1.97e-3 per channel, including after `Linear`
      through `then`.
    - `Categorical`, 20 keys and a 100-colour palette: 0 / 5.96e-8.
    - `Band` and `Point`, 1 to 1,000 keys on forward and reversed ranges:
      9.84e-5 / 1.05e-4 px.
  - Unit tests:
    - band geometry and ticks;
    - the symmetric diverging domain, colours darkening away from the midpoint,
      and the legend ticks;
    - generated colours at least 0.118 apart in OKLab (the test holds them to
      0.08);
    - the dictionary hook end to end (24 column bytes and no texture for a new
      key);
    - a null band key hidden, seeded against the compare.
  - `tests/ordinal_png.rs`: the three goldens below, plus legend-matches-data
    and null-absent checks.
  - Doctests: `Dictionary<i32>`, `encode_owned_key` (integer years on a `Point`
    axis, an enum legend), `KeyEncoded::domain`, `Band`, `Point`, `Diverging`
    and `Categorical`'s legend.
  - Test counts: gup-core lib **122** passed (3 ignored), from 108; integration
    **24** in 10 binaries (3 new in `ordinal_png`); doctests **13**, from 7. All
    pass on lavapipe too.
- **Browser** (`wasm-size/scatter`): two scenes, checked against the mirrors in
  wasm. One is an 11-key `Categorical` on a `Band` axis with a swatch legend and
  an empty-band null-key proof; the other is a `Diverging` scale with its ramp.
  `GUP PASS` on SwiftShader.
- **`PERF_BUDGETS.md`** re-records the WASM scatter at 294,333 B gz: +7,985 B of
  library and +14,642 B of scenes. gup-core is 252,621 B over bare wgpu.

### Definition-of-Done evidence

- **Rendered output, read by eye** (each also 0 of 324,000 pixels from its
  golden on lavapipe, max ΔE 1.6, 1.1 and 1.7):
  - `gup_core/categorical_species`: eleven bird species as tight clusters along
    a log-mass/wingspan diagonal.
    - The first eight are Okabe-Ito: Mallard is black, Kestrel yellow. Gull
      (dark magenta), Heron (olive) and Swan (light green) are generated.
    - A swatch legend to the right lists them in first-seen order, and every
      species is distinguishable from its neighbours.
    - The x axis runs 1 to 100k (Log's nice to decades), so the top-left and
      bottom-right are empty. That is the data's shape, not a defect.
  - `gup_core/diverging_anomaly`: 1950–2024 on a grey plot background. Early
    years are pale blue to near-white dots around 0, and recent years deepen
    through salmon to dark red near +1.1. The ramp legend (−1.0 … 1.0, 0 at its
    centre) is symmetric about 0, so its blue half has little data, as intended.
  - `gup_core/band_months`: twelve shaded bands labelled Jan … Dec (an enum's
    `Display`). Each holds a vertical strip of daily highs graded cool to warm,
    red in Dec–Jan, dark blue in Jun–Jul and near-white around 20 °C in Sep–Oct.
    Days without a month do not appear.
  - The browser PNG's two new panels: eleven colour columns under the key labels
    `Ash … Pine` with an 11-swatch legend, and a blue-to-red diverging series
    with its ramp bar.
- **Tests**: every gup-core test passes on Intel and on lavapipe (counts above).
  Per the orchestrator's rule, the root crate's tests are left to CI.
- **Lint**: `mask all-check` passes. Its clippy covers every workspace member
  and target, examples included, so the examples type-check.
  `scripts/clippy_wasm32.sh` and the harness's own wasm32 clippy and rustfmt are
  clean.
- **`mask old-path-loc`**: 28,910, unchanged. Outside `crates/gup-core`, only
  goldens under `tests/golden/gup_core/` and planning docs changed.

**`mask perf-budget`**: every check passed. GPU clock median was 583 MHz, above
the 350 MHz floor (S5a's runs were 483–517 MHz), so the timings are a little
faster than a floor-state run would be.

| Metric                        | Measured | Budget      |
| ----------------------------- | -------: | ----------- |
| `zoom.gpu_pass.median_ms`     |    3.140 | ≤ 4.668     |
| `zoom.gpu_pass.p95_ms`        |    6.322 | ≤ 7.875     |
| `zoom.cpu.median_ms`          |    0.667 | ≤ 0.912     |
| `zoom.cpu.p95_ms`             |    0.999 | ≤ 1.412     |
| uploads (columns, uniforms)   |  0, 88 B | exact       |
| `pipeline.link_create.median` | 0.914 ms | ≤ 1.056     |
| `pipeline.link_create.cold`   | 4.500 ms | ≤ 10        |
| `wasm.scatter.gz_bytes`       |  294,333 | re-recorded |
| `wasm.over_wgpu.gz_bytes`     |  252,621 | ≤ 400,000   |

## Retrospective

**Completed**: 2026-10-11

### Key Technical Learnings

#### A generic key cannot live in a type-erased store

- **Challenge**: AC5 asked for `Dictionary<K>` "behind the same `U32` format".
  But S4b's dictionaries lived in the `ColumnStore`, which knows formats, not
  row types.
- **Solution**: move each dictionary into its key channel's encoding, which
  knows `K`, and have the store hold codes. The hook then reads the labels from
  the dictionary beside it, with no plumbing through `Plot::resolve`.
- **Pattern**: put state with the type that can name it. A type-erased layer
  that has to hold typed data asks for `Box<dyn Any>` and downcasts.

#### A domain uniform can prove itself

- **Challenge**: "a new key changes only a uniform" is vacuous if nothing on the
  GPU reads the domain. A fixed LUT and no count would also write zero bytes.
- **Solution**: the categorical shader reads the key count, and codes outside it
  draw null. The AC1 test then fails unless the new key's point draws in its
  palette colour, so the count uniform must have reached the GPU.
- **Pattern**: when a test claims "only X was written", make X observable in the
  output too.

#### Proving a hidden row needs a position that would be visible

- **Challenge**: a null code through `Band` lands about 4×10⁹ px off-screen, so
  "the null row is not drawn" would pass even with no hiding mechanism.
- **Solution**: an empty-range `Band` maps every code, null included, to the
  same on-screen pixel; the dispatch confirms `[150, 150]`. Seeding the compare
  away (both in the emitted text and with an always-true select) fails the test.
- **Pattern**: S4b's lesson again (show the mechanism, not only the outcome),
  this time with a degenerate scale as the probe.

#### Bless leaves in-tolerance goldens alone

- **Challenge**: after changing the band shade from `#f1f3f7` to `#e4e7ee`,
  `GUP_BLESS=1` kept the old golden, because the change was within ΔE tolerance.
- **Solution**: delete the golden, then bless. Check the committed PNG's pixels
  (`magick … -format '%[pixel:p{x,y}]'`), not just the test result.

### Architectural Decisions

#### LUT read by texel for `Categorical`

- **Decision**: `Resource::Lut` and `textureLoad`, with the key count in
  `Params`.
- **Reasoning**: palettes of any length up to 2,048, and no change to the glue's
  resource handling. Okabe-Ito's extension is fixed at construction, so a
  growing domain never writes a texture.
- **Trade-off**: an unused sampler argument in the WGSL signature.
- **Future**: S7's `Theme` swaps the palette; `theme.null_color` replaces the
  `Params` field.

#### `NULL_CODE` compare, not a validity plane, for position keys

- **Decision**: the glue compares the code.
- **Reasoning**: codes already reserve a null, so a plane would duplicate
  information, cost a storage read and complicate the "no nulls, no bits" rule.
- **Future**: S6's `clip` contract carries both checks.

#### Symmetric automatic domain for `Diverging`

- **Decision**: fit to the farthest distance from the midpoint on both sides.
- **Reasoning**: equal colour strength for equal distance. A piecewise fit to
  each side's extent would make −0.2 as dark as +1.1.
- **Trade-off**: half the legend can be empty, as in `diverging_anomaly`.

#### Domain order by seeding the dictionary

- **Decision**: `KeyEncoded::domain` seeds codes, so there is no `u32` remapping
  link.
- **Reasoning**: S5a expected order to need a `u32 → u32` chain and a `Feeds`
  impl, with a trybuild case for what must stay rejected. Seeding makes code
  order equal domain order, so `Feeds` still admits only numbers.

### Development Workflow Insights

- **Never use `|` as a perl delimiter when the pattern has `\|`.** It becomes
  alternation, and the substitution rewrites the first "row" or "i" anywhere in
  the file. This happened three times early in the story: a copyright header,
  every "row" in `encoding.rs`, and `selection.rs`. Each was caught by review of
  `git diff` before committing. The Edit tool or a `#` delimiter is safe.
- **Disk.** The shared build directory's debug incremental caches grew 2–4 GB
  per round. Each pre-commit hook run with Rust changes lints the root crate's
  examples and leaves about 3.8 GB of them. Toggling `CARGO_INCREMENTAL`
  rehashes every test binary (about 2.2 GB of duplicates). Free space dipped to
  3.8 GB once; pruning idle incremental caches and the superseded test binaries
  (this checkout's only) brought it back to 8.2 GB. No `cargo clean`.
- **Synthetic data needs the same care as real data.** The first `band_months`
  indexed its Halton jitter by global row, which biased each month and drew an
  irregular "season". Indexing by day-within-month gave the smooth curve the
  data was meant to show. Read the PNG before trusting the test.
- **lavapipe differs in the last ulp** of an 8-bit texel read (5.96e-8). Exact
  assertions on GPU colour belong only to same-device comparisons.

### Follow-up Stories

None written. Each gap maps to a planned RFC-001 step, recorded in the RFC's
"S5b findings → Proposed adjustments":

- **S6 (marks)**: fill opacity and stroke (for S5a's opaque `sqrt_radius` discs
  and the near-white diverging midpoint), size-descending z-order, and a bar
  width that follows `Band::band_width()`.
- **S7 (guides, `Theme`, layers)**: legend placement and titles, axis titles,
  `theme.null_color`, collision-aware band labels, a heavier weight for year
  ticks on `Time` axes, and **one dictionary per shared key scale across
  layers**. Today two layers sharing a `ScaleRef<Categorical>` give one key two
  codes.
- **S10 (bar builder)**: `band_width()` and `KeyEncoded::domain` for sorted
  bars.
- The ASCII minus in tick labels, from the S5a review, was fixed here.
