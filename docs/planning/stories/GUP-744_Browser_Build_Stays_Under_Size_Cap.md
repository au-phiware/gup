---
implementer: opus
implementer-reason:
  attribution across the whole crate decides which cuts are safe; each candidate
  needs judgement about what a browser scatter may leave unlinked without a
  behaviour change (sonnet could apply the cuts once chosen)
touches:
  [
    crates/gup-core/PERF_BUDGETS.md,
    crates/gup-core/Cargo.toml,
    crates/gup-core/src/lod.rs,
    crates/gup-core/src/lod/,
    crates/gup-core/src/selection.rs,
    crates/gup-core/wasm-size/,
    scripts/wasm_size.sh,
  ]
---

# GUP-744: The Browser Build Stays Well Under Its Size Cap

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-19; AC2's floor not met, see Resolution) **Created**: 2026-11-19

## Context

RFC-001 §12 risk 10 caps gup-core at 400,000 B gz over bare wgpu
(`wasm.over_wgpu.gz_bytes`, `scripts/wasm_size.sh`, enforced by CI). GUP-451 cut
it from 356,914 to 263,544 B in October. Since then it has grown to 363,801 B
(scatter 396,161 B gz, bare wgpu 32,360), 36 KB of headroom, at roughly 8 KB a
week: level of detail (GUP-714 to 721, `Lod::Auto` linking the tier build, about
+6,000), accessibility (GUP-619 to 626), columnar input (GUP-628, 629, 664) and
interaction (GUP-676, 684, 685). Four more weeks at that rate breaks the cap
before release 0.1. Each re-recording so far has moved the scatter row, never
the cap, and nothing says when a story must trim instead.

`PERF_BUDGETS.md` "Where the bytes go (GUP-450)" is the last attribution (raw
bytes, 2026-10-18, 1.4 MB file); it predates all of the growth above. The
harness already builds with `opt-level = "z"`, LTO and one codegen unit, and
without the `svg` feature (`svg-export` is a separate +4,057 B row). GUP-716
found `prepare` copies generated per row type. This story re-attributes, cuts
what a browser scatter does not need, and writes the headroom policy.

## Read First

- `crates/gup-core/PERF_BUDGETS.md`: the WASM rows (lines 90 to 94), "WASM size"
  (from line 191), "Where the bytes go (GUP-450)" (from about line 244), and the
  re-record notes for GUP-451, GUP-715, GUP-718 and GUP-720 (lines 436 to 460
  and 575 to 630).
- `maskfile.md` `wasm-attrib` (line 112) and `scripts/wasm_attrib.sh`
  (`--row-type`); `scripts/wasm_size.sh`.
- `crates/gup-core/Cargo.toml` `[features]` (line 71: `svg`, `window`, `web`,
  `arrow`) and `crates/gup-core/wasm-size/scatter/Cargo.toml`.
- `docs/planning/stories/GUP-716_*.md` retrospective, the `prepare` copies.

## User Story

> "As a developer shipping a gup chart to the browser, I want the library to
> stay far enough under its size cap that new features do not force a choice
> between the cap and the feature, so that release 0.1 ships inside it."

## Acceptance Criteria

### AC1: Attribute the current scatter

- [x] Run `mask wasm-size` and `mask wasm-attrib` on current `main`; paste into
      the story: the crate table (top ten, raw B and share), the 20 largest
      monomorphised groups with instantiation counts, and the 30 largest
      functions and data segments.
- [x] Group `gup_core`'s bytes by module (`lod`, `selection`, `plot`, `scale`,
      `shader`, `describe`, `tooltip`, `gpu_pick`, `text_layer`, `input`, `web`,
      others) using the function names, and list each cut candidate with its
      measured gz saving (build with the candidate removed, `mask wasm-size`
      before and after, one candidate at a time).
- [x] Record the Debug/Display/panic-string share (`core::fmt`, `Debug` impls,
      `unwrap`/`expect` messages) and whether `wgpu` features or the harness's
      `wasm-bindgen`/`web-sys` features pull in unused code.

### AC2: Cut

- [ ] **Not met** (8,767 B gz; see Resolution and GUP-745). Over bare wgpu falls
      by at least **25,000 B gz** from the AC1 baseline (about 363,801 to at
      most 338,800). If AC1 shows safe candidates sum to less, the target is
      their sum with a **floor of 15,000 B**; below the floor, stop and report
      (Escalation).
- [x] No behaviour change: every native golden is byte-identical (no `BLESS`),
      `cargo test -- --test-threads=1` passes without test edits, and
      `mask wasm-browser` passes with identical pixel counts.
- [x] Public API unchanged: `cargo check --examples` and the doctests pass
      without edits, and `cargo public-api`-style diff, or a read of the `pub`
      items in the diff, shows no signature change.
- [x] A cut that makes code unreachable by default but available by a feature or
      by an opt-in is not allowed unless the default behaviour is unchanged for
      the scatter and for `Lod::Auto` on 10M rows (the LOD tests pass unedited).

### AC3: Headroom policy

- [x] `PERF_BUDGETS.md` gains a "Headroom policy" paragraph under "WASM size"
      with the rule in Decisions, the AC1 attribution (condensed, dated, with
      commit) replacing or sitting beside the GUP-450 table, and the cuts made
      with their measured gz.
- [x] `scripts/perf_budget.sh` still parses the file (`mask perf-budget` runs
      and prints the WASM rows), and no budget row value is edited by the worker
      (the orchestrator re-records).

## Decisions

- **Target**: at least 25,000 B gz off `wasm.over_wgpu` (floor 15,000), set to
  reach under 340,000 so that headroom is at least 60 KB, about seven weeks at
  the recent rate. Measured with `mask wasm-size` before and after on the same
  toolchain.
- **Policy to write** (change the numbers only by an AC1 finding):
  - Until 0.1 ships, `wasm.over_wgpu.gz_bytes` stays at most **360,000** (40 KB
    under the cap). A story may re-record the scatter row while over_wgpu is at
    most 360,000.
  - A story whose change takes over_wgpu above 360,000 trims at least its own
    growth in the same story, or records an owner-approved exception in its
    retrospective.
  - Above **380,000** no story merges without a trim story ahead of it; above
    the 400,000 cap CI fails, as now.
  - A story that adds more than 5,000 B gz states in its retrospective what
    linking it costs a scatter that does not use it.
- **Candidates, in the order to measure** (none is pre-approved; AC1 decides):
  generic code copied per row type (`prepare`, `Selection<T>` paths; GUP-716);
  the LOD tier build and size-ordered LOD paths that a layer under 200,000 rows
  never reaches (only if a runtime-size branch already exists, so that removing
  the link would need a feature: that is an escalation, not a cut);
  `Debug`/`Display` impls and `expect` strings (shorter messages are allowed,
  messages asserted by tests are not touched); the edge-solve reach index;
  text-shaping tables and font data; `describe`/tooltip code behind a branch the
  scatter never takes; wgpu features and limits requested but unused.
- Cut by restructuring (non-generic inner functions, `#[inline(never)]` on cold
  paths, `dyn` where the call is not hot) before cutting by feature.
- Build flags: `panic = "abort"`, `wasm-opt -Oz` and `strip` are tried only if
  the harness does not already use them; each counts as one candidate, and the
  flags must be ones `web.rs` can document for users.
- The cap and the `+2%` scatter tolerance stay as they are.

## Escalation Triggers

- A win needs a public API change, or removes or gates a user-visible feature
  (LOD, accessibility, hover, tooltips, columnar input).
- Safe candidates sum to less than 15,000 B gz.
- Anything in AC1 suggests the 400,000 cap itself should move, or that the
  harness (not the library) is mismeasuring.
- A cut changes a golden image, a browser pixel count or `Lod::Auto`'s default
  choice.
- A cut conflicts with a story in flight on `selection.rs` or `lod/`.

## Out of Scope

- Raising or questioning the 400,000 cap (owner decision).
- Re-recording budget rows in `PERF_BUDGETS.md` (the orchestrator does).
- New features behind flags to make the browser build smaller; a `lod` or `a11y`
  feature split needs its own story.
- Native binary size, SVG export (`svg-export`, GUP-557), compile time.
- Shrinking the Inter font subsets again (GUP-451 did the ASCII SemiBold cut).

## Technical Tasks

- [x] Baseline `mask wasm-size` and `mask wasm-attrib`; paste (AC1).
- [x] Group by module and measure each candidate alone (AC1).
- [x] Apply cuts, largest first, measuring after each; keep a cut only if it
      saves at least 1,000 B gz.
- [x] Run tests, goldens, `mask wasm-browser` (AC2).
- [x] Write the policy and the attribution into `PERF_BUDGETS.md` (AC3).
- [x] Propose the new re-record values in the retrospective for the
      orchestrator.

## Dependencies

### Prerequisite Stories

- GUP-721: LOD for Size-Ordered Layers ✅ (the last large linker of new code).
- GUP-450 ✅ and GUP-451 ✅: the attribution tooling and the earlier cuts.

### Enables Stories

- Release 0.1 (Track N3): a size budget that tolerates the remaining stories.

## Testing Strategy

- **Unit and integration tests**: the existing suite, unedited.
- **Visual validation**: native goldens byte-identical; `mask wasm-browser`
  pixel counts identical (before and after both pasted).
- **Performance**: `mask wasm-size` before and after on one toolchain;
  `mask perf-budget` shows no CPU or GPU row worse than its tolerance (cuts must
  not de-inline a hot path).

## Success Metrics

- [ ] **Not met** (356,270). `wasm.over_wgpu.gz_bytes` at most 338,800 (or the
      AC1-justified target, never above 348,800).
- [x] Headroom policy present in `PERF_BUDGETS.md` and followed by this story.

## Risk Assessment

- **Medium**: size cuts that de-inline hot paths or box generics cost CPU.
  _Mitigation_: `mask perf-budget` before and after; revert a cut that moves a
  row past tolerance.
- **Medium**: the savings are smaller than 25 KB because the growth is real
  code. _Mitigation_: the 15 KB floor and the escalation; the policy still
  lands.
- **Low**: another story lands size growth while this one runs. _Mitigation_:
  measure before and after on one base commit; the orchestrator re-records on
  the merge.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are checked (AC2's floor not met, recorded)
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified: goldens byte-identical, `mask wasm-browser`
      pixel counts identical to before
- [x] `mask wasm-browser` passes (wasm code is touched)
- [x] WASM size: `mask wasm-size` before and after pasted
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## AC1: Attribution (2026-11-19)

Measured at `e7dad2d4` (current `main`) in this worktree, rustc 1.93.1,
wasm-bindgen 0.2.113. `mask wasm-size`: scatter **397,397 B gz**, bare wgpu
32,360, **over bare wgpu 365,037** (the orchestrator's main checkout read
397,618 / 365,258; the 221 B difference is the checkout path inside panic
locations, see below). `mask wasm-attrib` (raw bytes, name section excluded):
file 1,357,534 B raw, name section 339,793 B.

Ten largest crates:

| Crate                  | Raw B   | Share of file |
| ---------------------- | ------- | ------------- |
| `gup_core`             | 382,629 | 28.19%        |
| `data`                 | 183,339 | 13.51%        |
| `core`                 | 138,131 | 10.18%        |
| `other`                | 66,876  | 4.93%         |
| `ttf_parser`           | 46,373  | 3.42%         |
| `wasm_bindgen_futures` | 43,803  | 3.23%         |
| `alloc`                | 34,608  | 2.55%         |
| `wgpu`                 | 21,519  | 1.59%         |
| `compiler_builtins`    | 17,629  | 1.30%         |
| `hashbrown`            | 15,101  | 1.11%         |

Twenty largest monomorphised groups:

| Group                                                                                               | Instantiations | Raw B  |
| --------------------------------------------------------------------------------------------------- | -------------- | ------ |
| `wasm_bindgen_futures::future_to_promise::{{closure}}::{{closure}}`                                 | 9              | 39,647 |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::fit_domains`                  | 6              | 22,236 |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::hit_items`                    | 3              | 19,278 |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::drain_feed`                   | 7              | 16,405 |
| `core::slice::sort::unstable::quicksort::quicksort`                                                 | 6              | 15,114 |
| `core::slice::sort::stable::quicksort::quicksort`                                                   | 5              | 8,692  |
| `<&T as core::fmt::Debug>::fmt`                                                                     | 35             | 8,022  |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::bind_axes`                    | 6              | 7,740  |
| `hashbrown::raw::RawTable<T,A>::reserve_rehash`                                                     | 11             | 7,277  |
| `core::slice::sort::stable::drift::sort`                                                            | 5              | 6,600  |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::row_fields`                   | 6              | 5,430  |
| `<S as gup_core::encoding::DynShaderFn>::params_bytes`                                              | 14             | 4,942  |
| `alloc::sync::Arc<T,A>::drop_slow`                                                                  | 36             | 4,401  |
| `<S as gup_core::encoding::DynShaderFn>::signature`                                                 | 14             | 3,282  |
| `alloc::raw_vec::RawVec<T,A>::grow_one`                                                             | 39             | 3,070  |
| `gup_core::plot::Plot::resolve_full::{{closure}}`                                                   | 9              | 2,927  |
| `alloc::vec::Vec<T,A>::push`                                                                        | 38             | 2,814  |
| `<gup_core::encoding::ColumnEncoding<A,S,D> as gup_core::encoding::DynColumnEncoding<T>>::evaluate` | 15             | 2,589  |
| `core::slice::sort::unstable::ipnsort`                                                              | 6              | 2,581  |
| `wasm_bindgen_futures::future_to_promise::{{closure}}`                                              | 9              | 2,531  |

Thirty largest functions and data segments:

| Item                                                                              | Raw B   | Share of file |
| --------------------------------------------------------------------------------- | ------- | ------------- |
| `data segment ".rodata"`                                                          | 183,202 | 13.50%        |
| `gup_core::plot::Plot::resolve_full`                                              | 31,346  | 2.31%         |
| `gup_core::render::Renderer::prepare`                                             | 14,997  | 1.10%         |
| `ttf_parser::Face::parse`                                                         | 13,194  | 0.97%         |
| `gup_core::shader::glue::emit`                                                    | 11,800  | 0.87%         |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::prepare`    | 10,362  | 0.76%         |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::edge_marks` | 10,142  | 0.75%         |
| `fontdue::font::Font::from_bytes`                                                 | 9,494   | 0.70%         |
| `gup_core::web::show::{{closure}}`                                                | 8,189   | 0.60%         |
| `gup_core::plot::Plot::resolve`                                                   | 7,842   | 0.58%         |
| `wasm_bindgen_futures::future_to_promise::{{closure}}::{{closure}}`               | 7,772   | 0.57%         |
| `gup_core::context::Context::new::{{closure}}`                                    | 7,631   | 0.56%         |
| `ttf_parser::tables::cff::cff1::_parse_char_string`                               | 6,873   | 0.51%         |
| `wasm_bindgen_futures::future_to_promise::{{closure}}::{{closure}}`               | 6,457   | 0.48%         |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::hit_items`  | 6,426   | 0.47%         |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::hit_items`  | 6,426   | 0.47%         |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::hit_items`  | 6,426   | 0.47%         |
| `gup_core::gpu_pick::<impl gup_core::plot::Plot>::request_pick`                   | 6,260   | 0.46%         |
| `gup_text::opentype::OtLayout::kern`                                              | 6,234   | 0.46%         |
| `gup_core::shader::link`                                                          | 5,828   | 0.43%         |
| `wasm_bindgen_futures::future_to_promise::{{closure}}::{{closure}}`               | 5,653   | 0.42%         |
| `wasm_bindgen_futures::future_to_promise::{{closure}}::{{closure}}`               | 5,281   | 0.39%         |
| `gup_core::column::ColumnStore::lod_tiers`                                        | 4,629   | 0.34%         |
| `dlmalloc::dlmalloc::Dlmalloc<A>::malloc`                                         | 4,586   | 0.34%         |
| `wasm_bindgen_futures::future_to_promise::{{closure}}::{{closure}}`               | 4,539   | 0.33%         |
| `<gup_core::plot::Plot as gup_core::chart::Chart>::handle`                        | 4,523   | 0.33%         |
| `wasm_bindgen_futures::future_to_promise::{{closure}}::{{closure}}`               | 4,093   | 0.30%         |
| `core::num::flt2dec::strategy::dragon::format_shortest`                           | 4,082   | 0.30%         |
| `compiler_builtins::math::libm_math::rem_pio2_large::rem_pio2_large`              | 3,988   | 0.29%         |
| `core::num::dec2flt::<impl core::str::traits::FromStr for f64>::from_str`         | 3,821   | 0.28%         |

Raw bytes find targets but do not rank them: repeated instantiations compress to
almost nothing. To rank by gz, each function body was compressed in file order
with a sync flush after it, and its share of the output attributed to it (a
deflate-window-aware estimate; the totals run about 13% over the real gz). By
that estimate the code section is 297,857 B gz and the data section 86,184, of
which the two Inter subsets are 40,213, the `f64::from_str` table
(`core::num::dec2flt`, 651 powers of five, 10,416 B of incompressible bytes)
10,135, the WGSL text 4,140, and vtables, panic locations and message strings
the rest.

`gup_core` by module (gz estimate, function names; inlined callees count toward
their caller): `selection` 32,248, `plot` 31,639, `scale` 18,074, `render`
16,386, `column` 11,774 (of which LOD's `lod_tiers` 2,333), `encoding` 10,012,
`shader` 9,575, `web` 7,603, `context` 4,435, `gpu_pick` 3,983, `target` 3,201,
`order` 3,005, `scope` 2,700, `pick` 2,202, `lod` 927 (all LOD-named functions
together, 5,726), `describe` 584, `input` 78; `tooltip` and `text_layer` have no
function of their own (inlined into `plot`). Of `selection`, the copies per row
type are cheap: the six `fit_domains` cost 2,048 together, 1,817 of it the first
copy, so moving `Selection`'s generic methods behind a T-free core would save
about 1–2 KB.

Debug, Display and panics: `core::fmt` 7,520 gz, float formatting (`flt2dec`)
4,915, `Debug` impls 4,611 (63 functions; the largest is wgpu's `TextureFormat`,
reached from gup-core's error messages and from wgpu), `Display` impls 2,261,
panic entry points 1,098. Panic locations name each source file by its absolute
path; dropping every path prefix would save 259 B gz, and the checkout path
moves the total by about 200 B. Neither is a cut, and the measurement is sound.

Features: `wgpu`'s `wgsl` and default features link no naga (no `naga` crate in
the file); gup-core requests only `TIMESTAMP_QUERY` as optional. The harness's
`web-sys` feature (`HtmlCanvasElement`) and gup-core's fifteen are all called by
`web.rs`. `ttf-parser` is already built without default features
(`opentype-layout` only). No unused feature pulls in code.

Build flags: `panic = "abort"` is already the wasm32-unknown-unknown default
(`rustc --print cfg`), and `wasm-bindgen --remove-name-section` already strips
names. `wasm-opt -Oz` (binaryen 129) takes the scatter 401 B gz down but bare
wgpu 1,432 down, so over bare wgpu rises by 1,031: not a cut.

Candidates, each measured alone with `mask wasm-size`'s default build (over bare
wgpu, B gz):

| Candidate                                                                              | Saving  | Verdict                                                                                   |
| -------------------------------------------------------------------------------------- | ------- | ----------------------------------------------------------------------------------------- |
| One unstable `u64` sort and one stable index sort for the crate (11 sort copies to 3)  | 8,767   | **Cut** (`85eab6c4`)                                                                      |
| `ttf-parser`'s CFF real-number parse without `f64::from_str` (its table and `dec2flt`) | ~15,865 | Needs a patched or forked dependency: **escalated**                                       |
| The edge solve's reach index (GUP-724), scan instead                                   | 3,061   | Not a cut: a sized layer's frame CPU goes back from 0.9 to 11.6 ms                        |
| wgpu `TextureFormat` names out of gup-core's error messages                            | 448     | Below the 1,000 B floor for a cut; changes messages                                       |
| `wasm-opt -Oz` on both harnesses                                                       | -1,031  | Not a cut                                                                                 |
| WGSL minified (comments, indentation)                                                  | ~400    | Below the floor                                                                           |
| `Selection` generic methods behind a T-free core                                       | ~1–2 KB | Below or at the floor; touches every `Layer` method and the public `DynColumnEncoding<T>` |

## Blocked (2026-11-19)

**Question**: the safe cuts sum to 8,767 B gz, under the 15,000 floor; the one
large remaining win, about 15,900 B gz, needs `ttf-parser` to stop calling
`f64::from_str`. Which route, if any, should this story take?

Where it stands: `85eab6c4` (the shared sorts) takes over bare wgpu from 365,037
to **356,270** B gz (scatter 388,630), under the 360,000 line of the headroom
policy. gup-core lib tests pass (339, 3 ignored, as before). AC1's attribution
is above; AC3 (the policy in `PERF_BUDGETS.md`) is not written yet.

The finding: `core::num::dec2flt` (the `f64::from_str` code, 3,024 B gz, and its
10,416 B table of powers of five, 10,135 B gz) is linked only by
`ttf_parser::tables::cff::dict::parse_float` (CFF DICT real numbers,
`s.parse()`), reached from `Face::parse`, which both fontdue and gup-text call.
The bundled Inter subsets are TrueType (`glyf`, no CFF table), so the path never
runs for them, but gup-text takes user fonts (`TextSystem::with_font`,
`add_fallback`), so CFF support is a feature. ttf-parser 0.25.1 (fontdue
0.9.5's) has the same `s.parse()`. Measured with a scratch `[patch]` of the
harness (reverted, never committed) replacing the parse with a crude
digit-and-exponent loop: over bare wgpu 356,270 to **340,405** (-15,865).

Options:

1. **Fork `ttf-parser` 0.21.1** into the workspace with an exact small parser
   for CFF reals (the nibble string is at most 64 characters: integer mantissa
   and power of ten, exact whenever the mantissa fits 2^53 and the power is
   within ±22, which covers CFF's DICT values; something exact for the rest),
   applied with `[patch.crates-io]` in the workspace and both harnesses. About
   -15,500 after the parser's own bytes. Caveat: `[patch]` does not reach users
   who build gup from crates.io, so the harness would measure a binary users do
   not get unless gup-text (and fontdue) depend on a published fork. A
   dependency decision, not mine.
2. **Upstream**: a PR to ttf-parser replacing `s.parse()`, taken when released
   (fontdue would need the bump too). Right for users, slow, lands nothing in
   this story.
3. **Stop here**: keep the 8,767 (over bare wgpu 356,270, inside the 360,000
   policy line, 43.7 KB under the cap), write the headroom policy and the
   attribution into `PERF_BUDGETS.md`, and file the CFF parse as GUP-745 (option
   2, with option 1 as its fallback if upstream stalls).

Not options: the reach index (-3,061, but a sized layer's CPU per frame goes
from 0.9 back to 11.6 ms), message edits for `TextureFormat` (-448), WGSL
minification (~-400), `wasm-opt -Oz` (+1,031 over bare wgpu), and a T-free
`Selection` core (~1–2 KB for a rewrite of every `Layer` method). Each other
candidate I can see is under the 1,000 B floor for a cut.

**Recommendation**: option 3 now, with GUP-745 for the upstream fix; option 1
only if the owner wants the bytes before upstream moves and accepts that the
harness then measures a patched dependency.

## Resolution (2026-11-19)

The orchestrator chose option 3: no dependency is forked or patched for the
harness, because the harness would then measure a build users do not get.

**AC2's floor was not met.** The safe cuts total **8,767 B gz** (over bare wgpu
365,037 to 356,270), against a floor of 15,000. The rest of the target is not in
gup's code: about 15,900 B gz is `f64::from_str` (`core::num::dec2flt`, its code
and its 10,416 B table of powers of five), linked only by ttf-parser's CFF DICT
real-number parse. That was measured with a scratch `[patch]` of the harness
(356,270 to 340,405), reverted and never committed. GUP-745 takes it on, in a
dependency that users get.

The rejected candidates and their measured costs are in the AC1 table above. The
reach index (GUP-724) costs 3,061 B gz and holds a sized layer's frame CPU at
0.9 ms instead of 11.6 ms, so it stays.

## Implementation Summary

- `crates/gup-core/src/sort.rs` (new, `pub(crate)`): `sort_u64` (the crate's one
  unstable sort), `sort_f64` (by `total_cmp`, through order-preserving `u64`
  bits), `sort_dedup_usize`, `sort_indices_by` (the crate's one stable sort,
  over indices with a `dyn` comparator) and `sort_by` on top of it. Three unit
  tests: keys order as `total_cmp` (both zeros, infinities, NaNs), `sort_by` is
  stable, and dedup.
- Call sites moved to it: `order.rs` and `lod.rs` (the packed `u64` sorts),
  `transform::box_stats`, `scale/symlog.rs`, `scale/legend.rs`,
  `builder/area.rs` and the edge cache in `selection.rs` (`f64`s),
  `Layer::write_brush` (brushed rows), `pick.rs` (`RowId`s, packed into `u64`
  while both halves fit 32 bits, else the stable sort), `gpu_pick.rs` (pick draw
  order, polled picks), `plot.rs` (focus order, tick-label boxes). `gup-text`'s
  kern lookups are inserted in place (a font has a handful). Each gives exactly
  the order of the sort it replaced; the reach index keeps its own unstable
  sort, whose tie order among equal positions it may depend on.
- `PERF_BUDGETS.md`: "Headroom policy (GUP-744)" under "WASM size", and "Where
  the bytes go (GUP-744)" beside GUP-450's tables. No budget row was edited.
- GUP-745 written.

Proposed re-record values for the orchestrator (this worktree, rustc 1.93.1):
`wasm.scatter.gz_bytes` 388,630, `wasm.over_wgpu.gz_bytes` 356,270,
`wasm.per_row_type.gz_bytes` 707, `wasm.svg_export.gz_bytes` 4,668
(`wasm.wgpu.gz_bytes` 32,360 unchanged).

### Definition-of-Done evidence

- `mask wasm-size`, before (`e7dad2d4`): scatter 397,397, over bare wgpu
  365,037. After: scatter 388,630, over bare wgpu 356,270, per row type 707, SVG
  export 4,668; the WASM rows pass `perf_budget.sh --wasm-only`.
- `mask wasm-browser`, before and after, identical pixel counts:
  `white=36795 grey=9607 coloured=17598`, `chunked-diff=0px/0`,
  `dict=798,797,806,852,320`, `dict-null-row-diff=0px`, `scales=2785`,
  `band=3059`, `diverging=3747`, `user=4148`, `interactive=ok`. The frame counts
  of the asynchronous steps vary from run to run (`gpu-pick-frames` 3 before, 4
  after; `gpu-hover-frames` 7 and 5); they count frames waited, not pixels.
- `mask merge-check`: every step PASS (story-ids, doc, docs-rs, gup-core 1,054
  passed and 8 ignored, tour-pages, tour-app, wasm-size, wasm-browser; dogfood
  skipped by default).
- No golden was blessed; `git diff --stat e7dad2d4 -- tests/golden` is empty.
- Public API: the diff adds only `pub(crate)` items and a private function; no
  `pub` signature changed.
- `mask all-check`: exit 0. `mask feature-matrix`: all 7 rows ok (483 s).
  `cargo test -p gup -- --test-threads=1`: 11 result lines, all ok, 154 passed,
  0 failed, 2 ignored. `cargo test -p gup-core --test lod` under lavapipe: 37
  passed, 0 failed. All of it was run at `240d0d02`.
- `mask perf-budget` was not run (it needs the display GPU's timing runs). The
  sorts this story moved are not on a per-frame path, and the zoom and LOD rows'
  sorts (`order.rs`, `lod.rs`) still call the same `sort_unstable` on the same
  `u64`s, now through a function marked `#[inline(never)]`.

## Retrospective

**Completed**: 2026-11-19

### Key Technical Learnings

#### Rank WASM bytes by gz, not raw

- **Challenge**: `twiggy`'s raw sizes pointed at `Selection<T, M>`'s copies per
  row type (six `fit_domains` of 3.7 KB each, 127 KB raw for the module), but
  `wasm.per_row_type.gz_bytes` said a row type costs about 1 KB gz.
- **Solution**: compress the code section function by function in file order,
  with a sync flush after each, and attribute each flush's output to its
  function. Repeated instantiations then show what they really cost: the five
  extra `fit_domains` copies, 230 B gz together. Sorts, whose copies differ by
  element type and comparator, cost about 1.5 KB gz each.
- **Pattern**: a monomorphised copy is cheap when it is nearly identical to
  another within deflate's 32 KB window, and dear when its code differs. Rank by
  that estimate; judge by `mask wasm-size`.

#### Look at the data segment's entropy

- **Challenge**: the data section was 86 KB gz, half of it not fonts.
- **Solution**: scan `.rodata` in 512 B blocks for high entropy. One 10 KB block
  was `core::num::dec2flt`'s table of powers of five, and `twiggy paths` showed
  that only ttf-parser's CFF reader calls `f64::from_str`. That single call is
  worth about 15.9 KB gz, more than every safe cut together.
- **Pattern**: an incompressible static table counts in full in the gz total.
  Find them by entropy, then find who links them.

#### Sorts that give exactly the same order

- **Pattern**: an unstable sort can replace another sort only where equal
  elements are indistinguishable (integers; `f64`s by `total_cmp`, where equal
  keys are equal bits). Any stable sort gives the stable order. Packing a key
  and an index into a `u64` (as `order.rs` already did) turns a stable sort by
  key into the shared unstable one.

### Architectural Decisions

#### One unstable and one stable sort for the crate

- **Decision**: `crate::sort` holds the crate's sorts; new code sorts through
  it.
- **Reasoning**: each new element type or comparator compiled a whole sort (11
  instantiations, about 15 KB gz).
- **Trade-off**: the cold stable sorts compare through a `dyn` closure, and
  `f64` sorts make an extra pass to convert to keys and back. Neither is on a
  per-frame path: focus order, pick draw order, polled picks, tick-label boxes,
  the edge cache (once per chunk and epoch), and brushes.
- **Future**: `clippy` cannot enforce it. A new `sort_by` in gup-core costs
  about 1.5 KB gz, and the policy's 5,000 B rule will not catch one.

#### No dependency patch for the harness

- **Decision** (orchestrator): the harness measures what crates.io users get, so
  `[patch]` is not a cut.
- **Future**: GUP-745 removes the float parser in a dependency users get.

### Development Workflow Insights

- Scratch measurements (the ttf-parser patch, the reach index stubbed, the
  `TextureFormat` names removed) were made in the worktree and reverted with
  `git checkout`. Each took one incremental LTO build, about 25 s with two jobs.
- `wasm-bindgen --out-name` changes import module names and so the size: a quick
  measuring script must use the same name as `wasm_size.sh`
  (`gup-core-scatter`), or its numbers are about 1 KB gz off.
- The `--test-threads=1` lib suite (339 tests) took 63 s; `mask merge-check`
  took about 20 minutes with two jobs.

### Follow-up Stories

1. **GUP-745: Fonts Without a Float Parser in the Browser Build**: remove
   ttf-parser's `f64::from_str` from the browser build (about 15,900 B gz) in a
   dependency that users get. An upstream PR needs the owner's approval.
