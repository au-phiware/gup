---
implementer: sonnet
implementer-reason:
  the three cuts, their order and decision rules are settled in Decisions, and
  the Selection split is specified down to the types; gz yield is the only
  unknown and is an escalation
---

# GUP-451: Cut gup-core's WASM Size Below 320 KB

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-18

## Context

gup-core costs 356,914 B gz over bare wgpu against RFC-001 §12 risk 10's 400,000
B cap: 43 KB of headroom with S9 to S12 still to come (GUP-450's Context has the
numbers). This story makes three cuts, chosen from GUP-450's attribution, and
brings the cost to at most **320,000 B gz**. That leaves about 80 KB for S9 to
S12 at the rate S8 grew.

The cuts and the evidence for each:

1. **The harness has no release profile.** Both `wasm-size` harnesses are their
   own workspaces with no `[profile.release]`, so they build at `opt-level = 3`
   with 16 codegen units and no LTO. That is cargo's default, not what a size-
   conscious wasm user ships. The baseline gets the same profile, so the
   comparison stays fair. `wasm-opt` is not used: RFC-001 "S3 findings" measured
   that `wasm-opt -Oz` shrinks raw size by 14–15% and leaves gzip size unchanged
   or larger.
2. **SemiBold is subset like Regular, but only used for years.** GUP-436 bundled
   Inter SemiBold with Regular's 593-glyph character list (30,967 B gz). Its
   only use in the library is the major `Time` tick label (a year). The scribe
   ran the dev shell's `pyftsubset` with `mask subset-inter`'s options on
   printable ASCII plus U+2009, U+202F and U+2212. That subset is 18,636 B raw
   and 9,990 B gz, which saves about 21 KB gz.
3. **`Selection<T, M>` is monomorphised per row type.** Its `Layer` methods take
   159,137 B of raw code across the harness's 7 instantiations, and almost none
   of that code touches `T`. `prepare` reads `T` only through `Encoding<T>`,
   whose column variant is a `Box<dyn DynColumnEncoding<T>>`, and that trait
   uses `T` in one method, `evaluate`. `M` contributes only associated
   constants. Moving the bodies into non-generic functions over a `T`-free view
   makes them one copy per program.

The owner allowed further techniques: feature-gating and lazy font weights. This
story does not use them. Feature-gating moves no measured bytes, because the
harness, being the browser smoke test, turns every feature on. Fetching a font
weight lazily would make wasm output depend on the network, unlike native, and
cut 2 already removes two thirds of SemiBold's bytes.

## Read First

- [GUP-450](GUP-450_WASM_Size_Attribution_And_Row_Type_Gate.md) "Context" and
  its retrospective; `PERF_BUDGETS.md` "Where the bytes go (GUP-450)".
- `crates/gup-core/src/selection.rs` lines 100–580 (`Selection`, `evaluate`,
  `glue`, `chunk_uniform_bytes`) and 576–1160 (`Layer` and its impl).
- `crates/gup-core/src/encoding.rs` lines 940–1180 (`Encoding<T>`,
  `DynColumnEncoding<T>` and its three implementors).
- `crates/gup-core/src/channel.rs` lines 330–350 (`Mark`).
- `crates/gup-text/src/font.rs` lines 25–40 and 250–300, and
  `crates/gup-text/src/system.rs` lines 215–290.
- `maskfile.md` "subset-inter" (lines 105–135);
  `crates/gup-text/fonts/inter-subset.txt`.
- `crates/gup-core/PERF_BUDGETS.md` "WASM size" (lines 145–198).

## User Story

> "As a web developer using gup-core, I want the library to cost as little
> download as it can, and to cost little more for each new row type I plot, so
> that a chart page stays light as my app grows."

## Acceptance Criteria

### AC1: Spike — release profile

- [x] Measure `mask wasm-size` with each of the three profiles in Decisions,
      applied to both harnesses, and record all three rows (wgpu, scatter,
      over_wgpu, per_row_type) in the story.
- [x] Apply the profile that the decision rule picks to both harness
      `Cargo.toml`s, with a comment naming this story and the measurement.

#### AC1 measurements (2026-10-18)

| Profile | wgpu gz | scatter gz | over_wgpu gz |           per_row_type gz |
| ------- | ------: | ---------: | -----------: | ------------------------: |
| default |  41,712 |    398,627 |      356,914 |                     3,674 |
| s+LTO   |  30,460 |    340,719 |      310,259 | 2,531 (343,250 - 340,719) |
| z+LTO   |  32,360 |    324,952 |      292,592 | 1,078 (326,030 - 324,952) |

`z` is 5.7% under `s` (rule: at least 1%), so both harnesses use `z`+LTO,
`codegen-units = 1`. The target (320,000) is met by this cut alone.

### AC2: SemiBold subset

- [x] `crates/gup-text/fonts/inter-semibold-subset.txt` lists `0020-007E`,
      `2009`, `202F` and `2212`, with comments in the style of
      `inter-subset.txt` saying why. `mask subset-inter` subsets each weight
      with its own list. `Inter-SemiBold-Subset.ttf` is regenerated with it, and
      is within 1% of 18,636 B raw and 9,990 B gz.
- [x] gup-text's test `semibold_subset_covers_what_regular_covers` is replaced
      by `semibold_subset_maps_exactly_its_list`, which mirrors the Regular
      subset's test.
- [x] A new test in gup-core, `major_tick_labels_fit_the_semibold_subset`,
      generates `Time` ticks over 1 month, 1 year, 10 years and 200 years,
      starting in 1850 and in 1970, and asserts that every major label's
      characters map to non-notdef glyphs in `Font::inter_semibold()`.
- [x] Every golden passes **without** `GUP_BLESS`, including `time_years.png`
      and `time_tick_hierarchy.png`, which draw SemiBold: the glyphs are the
      same outlines.
- [x] The docs on `Weight::SemiBold` and `Font::inter_semibold` say that the
      bundled SemiBold covers printable ASCII and U+2009, U+202F and U+2212
      only, other characters draw as the missing-glyph box, and
      `TextSystem::with_fonts` takes a fuller face.
      `git grep -n -i semibold -- crates docs/*.md` finds no other statement
      that SemiBold covers what Regular covers.

- [x] A SemiBold run containing a character outside the subset draws that
      character from the Regular face, never as a missing-glyph box. A test with
      a non-ASCII SemiBold label (for example "2024–Q1", with an en dash) checks
      that every glyph has ink (orchestrator, 2026-10-18).

### AC3: `Selection` split

- [ ] The `Layer` bodies move to non-generic functions in a new
      `crates/gup-core/src/layer.rs`, as Decisions specifies. Each method of
      `impl<T, M> Layer for Selection<T, M>` is at most 15 lines and only
      forwards. The code that touches `T` stays generic: running accessors,
      `row_any`, `dispatch`, `tooltip_text`, building an `AxisEncoding<T>` in
      `bind_axes`, and `append`.
- [ ] `mask wasm-attrib`'s `== monos` section shows no `Selection<T,M>` method
      group, `Layer` impl or inherent, with a mean instantiation over 1,000 B
      raw. The builder methods users call (`new`, `attr`, `attr_axis`, `on`,
      `tooltip`, `z`, `retain`, `legend_title`) are exempt.
- [ ] Behaviour is unchanged: `cargo test -p gup-core -- --test-threads=1`
      passes with no test edited except for type names; every golden passes
      without `GUP_BLESS`; `tests/zoom_uploads.rs` and
      `reference_glue_matches_fixtures` pass unchanged.

### AC4: Targets and budgets

- [ ] After all three cuts, `mask wasm-size` reports
      `wasm.over_wgpu.gz_bytes ≤ 320,000`.
- [ ] `wasm.per_row_type.gz_bytes` after cut 3 is at most 50% of its value after
      cuts 1 and 2 (same profile, measured in this story).
- [ ] `PERF_BUDGETS.md`: re-record `wasm.wgpu`, `wasm.scatter` and
      `wasm.per_row_type`, and set the per-row-type check to `+25%`. Keep
      `wasm.over_wgpu`'s check at `<= 400000`. Add a History entry with the gz
      saving of each cut, measured one at a time in Decisions' order, and update
      the "WASM size" text: the harnesses' profile, and the SemiBold subset
      line.
- [ ] `scripts/wasm_size.sh` also prints the SemiBold subset's size, next to the
      Regular subset's line.
- [ ] `mask wasm-browser` and `mask perf-budget` pass. Native timings are not
      re-recorded unless they fail.

### AC5: Users can reproduce the number

- [x] The `web` module docs (`crates/gup-core/src/web.rs` header) gain a
      "Release profile" paragraph with the profile AC1 chose, saying that
      `PERF_BUDGETS.md`'s WASM sizes are measured with it, and how many bytes it
      saved.

## Implementation Results (2026-10-18)

Orchestrator decisions overrode the story's order: AC1, AC2 first, then
`.rodata` attribution; AC3 conditional; PERF_BUDGETS values are recorded by the
orchestrator, not here.

### Savings, one cut at a time (`wasm.over_wgpu.gz_bytes`)

| Step                                                | over_wgpu gz |  Saved |
| --------------------------------------------------- | -----------: | -----: |
| Start (default profile)                             |      356,914 |      - |
| Cut 1: `z`+LTO, `codegen-units = 1`                 |      292,592 | 64,322 |
| Cut 2: ASCII SemiBold subset (10,009 B gz)          |      271,207 | 21,385 |
| Cut 4: `to_ascii_uppercase` (no Unicode case table) |      263,543 |  7,664 |

Target 320,000 met with 56,457 B to spare. `wasm.per_row_type.gz_bytes` is 946 B
(was 3,674). Subset regenerated: 18,636 B raw (exact), 10,009 B gz (9,990
expected, +0.2%).

### AC2 extras

- `Font` gained a private `fallback`; `Font::inter_semibold()` falls back to the
  Regular subset per character (`face_for`) in `measure`, `walk` (so layout, ink
  bounds and the atlas) and `has_glyph`. Kerning is skipped across a face
  change. New public `Font::has_own_glyph` (ignores the fallback); the gup-core
  test uses it. Tests: `semibold_subset_maps_exactly_its_list`,
  `semibold_draws_what_it_lacks_from_regular` (gup-text),
  `major_tick_labels_fit_the_semibold_subset` (gup-core `scale`). All goldens
  pass unblessed.
- SVG output embeds the SemiBold subset only (no fallback in a viewer's
  `@font-face`); documented on `SvgOptions::semibold_font`.

### `.rodata` attribution (after cuts 1 and 2; 181,464 B raw, was 230,826)

Parsed from the shipped wasm's data segment (scatter 181,464 B; bare wgpu 10,944
B, so gup-core's share is 170.5 KB):

| Part                                                                            |    Raw bytes |
| ------------------------------------------------------------------------------- | -----------: |
| Inter Regular subset                                                            |       61,260 |
| Inter SemiBold subset (was 62,308 before cut 2)                                 |       18,636 |
| Text: WGSL library and mark shaders                                             |       17,679 |
| Text: messages and identifiers                                                  |       14,618 |
| Text: wgpu / WebGPU enum and feature names                                      |        7,582 |
| Text: source paths in panic locations                                           |        7,031 |
| Binary: Unicode `to_uppercase`/`to_lowercase` tables                            | about 14,000 |
| Binary: other tables (core printable/grapheme data, libm, vtables, relocations) | about 40,000 |

Cuts made from it: the 14 KB case-conversion table was only pulled in by two
`str::to_uppercase()` calls in `selection.rs` on ASCII channel names
(`check_module_paths`, an error detail); they now use `to_ascii_uppercase()`
(same output for ASCII names). That is 7,664 B gz. No rendered output depends on
it. The rest is the two fonts (the Regular subset's 593 characters are GUP-407's
choice) and strings; nothing else is a safe cut.

### Checks the orchestrator asked for

- ttf-parser features: fontdue already sets `default-features = false` and
  `opentype-layout` only (needed for kerning); `glyph-names`, `apple-layout` and
  `variable-fonts` are already off. CFF (12,243 B raw, 25 functions) has no
  feature switch, so it needs a fork: still out of scope.
- Export code: `SvgTarget::render` does not reach the harness (all `svg` symbols
  total 1,192 B: only helpers); `ImageTarget` is 6,575 B raw, used by the
  harness's scenes. Nothing to cut.

### AC3 skipped

Condition: target unmet after steps 1 and 2 and `mask wasm-attrib` predicting at
least 8,000 B gz. The target is met (263,543 vs 320,000) and the measured
per-row-type cost is 946 B gz, so the split would save little. Not done; the
`DynColumnFn` supertrait and `layer.rs` remain a design for a time when the
per-row-type metric grows.

### AC4

`wasm.over_wgpu.gz_bytes` 263,543 <= 320,000 met. The per-row-type "50% of after
cuts 1 and 2" check is moot (AC3 skipped). `PERF_BUDGETS.md` values, History and
the per-row-type check are for the orchestrator to record. `wasm_size.sh` prints
the SemiBold subset line. `mask wasm-browser` prints identical pixel numbers
before and after (`white=36234 grey=9516 coloured=18250 ... interactive=ok`).

## Decisions

### Order and measurement

Make the cuts in the order 1, 2, 3, and run `mask wasm-size` after each. Each
cut's saving is the difference from the previous step on the same tree, so the
three savings add up to the total.

### Cut 1: release profile

Three candidates, each written into both harnesses' `Cargo.toml`:

| Name    | `[profile.release]`                                  |
| ------- | ---------------------------------------------------- |
| default | none (today)                                         |
| `s`+LTO | `opt-level = "s"`, `lto = true`, `codegen-units = 1` |
| `z`+LTO | `opt-level = "z"`, `lto = true`, `codegen-units = 1` |

Rule: take the candidate with the smallest `wasm.over_wgpu.gz_bytes`. Between
`s`+LTO and `z`+LTO, take `z` only if it is at least 1% smaller than `s`,
because `s` keeps more speed-oriented optimisation. If neither beats the
default, keep the default and say so. `panic` is not set (`abort` is already the
wasm32-unknown-unknown default). `strip` is not set (wasm-bindgen already
removes names in `wasm_size.sh`).

### Cut 2: SemiBold coverage

Printable ASCII (U+0020–U+007E), thin space U+2009, narrow no-break space U+202F
and minus U+2212, with the same `pyftsubset` options as Regular (`kern`, `tnum`,
no hinting, notdef outline, name IDs 0–7, 13, 14). That covers every label the
library draws in SemiBold today (years, with U+2212 for negatives) and ASCII
titles that users set in SemiBold. `TextSystem` is unchanged: a missing glyph
draws as the box, as it does for Regular.

### Cut 3: the split

New module `crates/gup-core/src/layer.rs` (`pub(crate)`), with the `Layer` trait
staying in `selection.rs`:

```rust
/// A mark's associated constants as a value, so non-generic code can read them.
#[derive(Copy, Clone, Debug)]
pub(crate) struct MarkInfo {
    pub name: &'static str,
    pub module: &'static WgslModule,
    pub channels: &'static [ChannelDesc],
    pub vertices_per_instance: u32,
    pub antialias: Antialias,
    pub geometry: Geometry,
    pub hit: HitShape,
}
impl MarkInfo { pub(crate) const fn of<M: Mark>() -> Self { /* copy M's consts */ } }

/// A channel's encoding without its row type.
pub(crate) enum EncodingRef<'a> {
    Const(ConstValue),
    Derived(&'a (dyn Fn() -> ConstValue + Send + Sync)),
    Column(&'a dyn DynColumnFn),
}
pub(crate) enum EncodingMut<'a> {
    Const(ConstValue),
    Derived(&'a (dyn Fn() -> ConstValue + Send + Sync)),
    Column(&'a mut dyn DynColumnFn),
}

/// Everything in a `Selection` that does not depend on `T`.
pub(crate) struct LayerState {
    pub mark: MarkInfo,
    pub released: usize,
    pub retain: Retain,
    pub z: i32,
    pub axes_bound: u64,
    pub legend_titles: Vec<Option<String>>,
    pub columns: Option<ColumnStore>,
    pub order: Option<DrawOrder>,
    pub max_chunk_rows: Option<u32>,
    pub gpu: Option<(Arc<LayerGpu>, Luts)>,
}
```

`Selection<T, M>` becomes
`{ rows: Vec<T>, encodings: Vec<Option<Encoding<T>>>, axes: Vec<Option<AxisFn<T>>>, handlers, tooltip, state: LayerState, _mark }`.
Its public API does not change.

`encoding.rs` splits `DynColumnEncoding<T>` into a `T`-free supertrait and a
remainder:

```rust
/// The part of a column encoding that does not depend on the row type.
pub trait DynColumnFn: Send + Sync {
    fn links(&self) -> Vec<&dyn DynShaderFn>;
    fn fit(&mut self, stats: Option<ColumnStats>) -> Result<()>;
    fn image(&self, extent: (f64, f64)) -> Option<(f64, f64)>;
    fn cpu_eval(&self, v: f64) -> Option<f64> { self.image((v, v)).map(|(a, _)| a) }
    fn legend(&self) -> Option<crate::Legend>;
    fn apply_theme(&mut self, theme: &Theme);
}
pub trait DynColumnEncoding<T>: DynColumnFn {
    fn evaluate(&mut self, rows: &[T]) -> Result<ColumnData>;
}
```

`DynColumnFn` is re-exported at the crate root next to `DynColumnEncoding`.
`&dyn DynColumnEncoding<T>` becomes `&dyn DynColumnFn` by trait upcasting
(stable since Rust 1.86; the toolchain is 1.93). `ColumnEncoding<A, S, D>` and
`KeyEncoded<K, S>` implement `DynColumnFn` without a `T`, so those methods also
stop being copied per row type.

The non-generic functions in `layer.rs` take
`(&mut LayerState or &LayerState, &[Option<EncodingRef<'_>>] or &mut [Option<EncodingMut<'_>>], …)`:
`prepare`, `glue`, `check_module_paths`, `chunk_uniform_bytes`, `overhang`,
`legends`, `begin_fit`, `revision`, `apply_theme`, `fit` (the stats half of
`fit_domains`), `store_columns` (the store-building half of `evaluate`, given
the `Vec<ColumnData>` the generic half produced), `release` (given the row
count, returning whether to drop the rows) and `hit_items`. `hit_items` takes
`Option<&mut dyn FnMut(usize) -> Result<ColumnData>>` for the case where values
come from running accessors over rows.

The generic side has one helper,
`fn erased(&self) -> Vec<Option<EncodingRef<'_>>>` (and `erased_mut`); the
allocation is at most one entry per channel. `layer.rs`'s module docs and the
`Layer` trait's docs say that new `Layer` behaviour goes into `layer.rs` as
non-generic code, and that the `Selection` impl only forwards. GUP-450's
per-row-type metric enforces this.

## Escalation Triggers

- After all three cuts, `wasm.over_wgpu.gz_bytes > 320,000`. Report each cut's
  saving and `mask wasm-attrib`'s top groups; do not add a fourth cut.
- Any golden changes after cut 2 or cut 3.
- The split needs `unsafe`, cloning rows or columns, or a public API change
  beyond the `DynColumnFn` supertrait.
- Cut 1's chosen profile slows `mask wasm-browser` past CI's job timeout, or
  makes a browser step flaky.
- The regenerated SemiBold subset is more than 1% off the sizes in AC2.

## Out of Scope

- Cutting ttf-parser's unused CFF code, or swapping fontdue: no switch exists
  for it, so it would need a fork.
- Shrinking the harness's own scenes: they are the browser smoke test.
- Lowering the 400,000 B cap: the headroom is for S9 to S12.
- A per-mark (`M`) size gate.

## Technical Tasks

- [ ] AC1: three `mask wasm-size` runs; apply the rule; record.
- [ ] AC2: subset list, `mask subset-inter`, regenerate, tests, docs.
- [ ] AC3: `layer.rs`; `DynColumnFn`; move bodies; shims; docs.
- [ ] AC3: `mask wasm-attrib` and check the `monos` rule.
- [ ] AC4: budgets, History, `wasm_size.sh` SemiBold line; `wasm-browser`,
      `perf-budget`.
- [ ] AC5: `web.rs` docs.

## Dependencies

### Prerequisite Stories

- GUP-450: WASM Size Attribution and a Per-Row-Type Gate 📋 — `mask wasm-attrib`
  and the `wasm.per_row_type.gz_bytes` metric.
- GUP-436: Semibold Time Year Ticks ✅ — the SemiBold face being re-subset.
- GUP-445: CPU Picking ✅ — `hit_items`, the largest method after `prepare`.

### Enables Stories

- GUP-452 to GUP-456 (RFC-001 S9): the headroom they need, and a split in which
  their new `Layer` methods cost nothing per row type.

## Testing Strategy

- **Unit and integration**: the full gup-core and gup-text suites, unchanged
  apart from AC2's two tests; every golden without blessing.
- **Size**: `mask wasm-size` after each cut; `mask wasm-attrib` for the `monos`
  check.
- **Browser**: `mask wasm-browser` with the chosen profile.
- **Performance**: `mask perf-budget` (native; it should not move, since the
  split adds one small allocation per layer per prepare).

## Success Metrics

- [ ] `wasm.over_wgpu.gz_bytes ≤ 320,000` (from 356,914).
- [ ] The marginal row type costs at most half what it did after cuts 1 and 2.
- [ ] No golden, glue fixture or upload count changes.

## Risk Assessment

- **Medium**: the split's gz saving is estimated, not measured. Duplicate
  instantiations far apart in the code section compress independently, so the
  estimate is about 25–40 KB gz. Cuts 1 and 2 alone are expected to get within
  about 15 KB of the target. _Mitigation_: the per-cut measurements and the
  escalation trigger.
- **Low**: `opt-level = "z"` can slow wasm CPU paths (pick index, text layout).
  _Mitigation_: the rule prefers `s` unless `z` is clearly smaller; browser
  steps have timeouts that would show a large slowdown.
- **Low**: users who style non-ASCII text SemiBold get boxes. _Mitigation_: the
  docs, and `with_fonts`.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Goldens pass without `GUP_BLESS`; `mask wasm-browser` passes
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
