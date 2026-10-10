# GUP-419: RFC-001 Step S5b: Colour Scales, the Dictionary Hook, Band and Point

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-10

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

- [ ] A `ShaderFn` that needs dictionary-driven domain information (key count,
      and the keys themselves in first-seen order for legend labels) receives
      them through a dedicated hook, analogous to how `fit_domain` already
      receives a numeric extent — this story's hook replaces or extends
      `fit_domain` for dictionary-backed channels; document which.
- [ ] A new dictionary key on a resolved chart's data changes only a uniform
      (the domain grows), with no column bytes rewritten for rows that already
      had codes — proved by the existing upload-counting infrastructure
      (`Context::write_buffer`/`Upload` kinds from S0b/S4a), not merely
      asserted.
- [ ] A test exercises the hook end to end: a `Categorical`-encoded column grows
      its dictionary (via append or a second resolve with new rows), and the
      scale's domain/legend reflect the new key without a column rewrite.

### AC2: Full `Categorical` colour scale

- [ ] `Categorical` maps dictionary codes to colours from the Okabe-Ito palette
      (RFC default), now sized to the dictionary's actual key count, not a fixed
      cycle of 8.
- [ ] Palettes longer than 8 are supported: the implementation picks a
      length-generic uniform array or a LUT texture (reusing the `Resource`
      mechanism `Sequential` already proves) and documents the choice and its
      size limit, if any.
- [ ] `Categorical::legend()` (per `ColorScale`'s trait method, RFC §5) returns
      labelled swatches in the dictionary's first-seen order, each swatch's
      colour matching what the GPU renders for that key — checked by a test that
      renders a legend and the data with the same scale and compares colours.
- [ ] A golden render (read by eye, and checked through the
      `gup-visual-regression` harness's perceptual tolerance — never a
      byte-exact comparison to a PNG blessed only on this machine) shows a
      scatter chart with more than 8 distinct categories, each with a visibly
      distinct colour, and a legend whose swatches and labels match.

### AC3: `Diverging` colour scale

- [ ] `Diverging` implements `ColorScale` + `CpuMirror`: two colour ramps
      meeting at a configurable midpoint domain value, via a LUT texture (the
      same mechanism as `Sequential`).
- [ ] `conformance` covers `Diverging` within 1/255 per channel, including
      values at, above and below the midpoint.
- [ ] A golden render shows a chart with a signed quantity (e.g. temperature
      anomaly) coloured by `Diverging`, with visually distinct colours on each
      side of the midpoint and a legend gradient bar matching the data colours
      at both extremes (reusing the `ItemKind::Gradient` scene item from S3).

### AC4: `Band` and `Point` position scales

- [ ] `Band` implements `PositionScale` (`In = u32`, `Out = Px`) over dictionary
      codes, with configurable padding (inner/outer) per category, and a
      `band_width()` (or equivalent) accessor so a mark (for example, a future
      bar mark) can size itself to the band.
- [ ] `Point` implements `PositionScale` over dictionary codes, placing each
      category at the centre of its slot with no width (for scatter-style
      categorical axes).
- [ ] A null key on a `Band`/`Point` X or Y channel is hidden (not drawn), using
      whichever mechanism the implementer chooses (a validity plane on `U32`
      geometry columns, or a `NULL_CODE` comparison in the glue) — document the
      choice and why.
- [ ] `conformance` covers `Band`/`Point` positions against their CPU mirrors
      within 0.25 px across a dictionary of several categories, including a
      null-key row proven hidden (not merely "renders at some position").
- [ ] A golden render shows a categorical x-axis (`Band` or `Point`) with tick
      labels drawn from the dictionary's keys (not numeric codes), read by eye
      through the harness tolerance.

### AC5: Generic `Dictionary<K>`

- [ ] A generic `Dictionary<K>` (or equivalent) supports `Hash + Eq + Display`
      keys beyond `&str` (for example, integers or an enum), behind the same
      `U32` column format, with its own key-accessor entry point(s) analogous to
      `encode_key`/`encode_nullable_key`.
- [ ] A test (unit or doctest) drives `Categorical` or `Band` from a non- string
      key type and confirms correct domain ordering and label formatting (via
      `Display`).

### AC6: Browser and old-path freeze

- [ ] `mask wasm-browser` is extended to exercise `Diverging`, the full
      `Categorical` (with a palette larger than 8, and its legend), and
      `Band`/`Point` (including a hidden null key), and passes on
      WebGPU/SwiftShader.
- [ ] No file outside `crates/gup-core` (and planning docs) changes;
      `mask old-path-loc` reports the same count as before this story.

## Technical Tasks

- [ ] Design and implement the dictionary hook on `ShaderFn` (or a sibling
      trait), replacing/extending `fit_domain` for dictionary-backed channels,
      and wire it through `Plot::resolve`.
- [ ] Rebuild `Categorical` on the hook: dictionary-sized palette (uniform array
      or LUT, decide and document), `legend()`, and its WGSL module.
- [ ] Implement `Diverging` (WGSL module, CPU mirror, LUT-backed) and its
      `legend()`.
- [ ] Implement `Band` and `Point` (WGSL modules, CPU mirrors) over dictionary
      codes, including padding/width for `Band`.
- [ ] Resolve the null-key-on-position question (validity plane vs. `NULL_CODE`
      compare) for `U32` geometry columns and implement it in the glue emitter.
- [ ] Add a generic `Dictionary<K>` and its key-accessor entry points.
- [ ] Extend `conformance.rs` with `Diverging`, `Categorical` (dictionary-
      sized), `Band` and `Point` cases, including the null-key case.
- [ ] Add the dictionary-hook end-to-end test (AC1) and the legend-matches-
      data-colour test (AC2/AC3).
- [ ] Extend the `mask wasm-browser` harness scene(s) for AC6.
- [ ] Re-run `mask old-path-loc` and confirm it is unchanged.

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

- [ ] A new dictionary key grows a scale's domain with 0 column bytes rewritten,
      measured through the existing upload counters.
- [ ] `Categorical`'s legend colours match the GPU-rendered data colours for
      every category, including palettes larger than 8.
- [ ] A null key on a `Band`/`Point` position channel is proven hidden, not
      merely assumed from the render.
- [ ] `mask wasm-browser` and `mask old-path-loc` are unaffected beyond the
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

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read) for AC2, AC3
      and AC4
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
