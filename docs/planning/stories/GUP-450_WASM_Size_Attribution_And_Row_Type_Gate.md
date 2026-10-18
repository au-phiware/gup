---
implementer: sonnet
implementer-reason:
  tool choice is a spike with a decision rule, the gate's scene and metric are
  specified; follows scripts/wasm_size.sh and the perf_budget.sh table
---

# GUP-450: WASM Size Attribution and a Per-Row-Type Gate

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-18

## Context

`mask wasm-size` on `main` at 7fb1a99 (2026-10-18) measures the reference
scatter at 398,627 B gz, and gup-core at 356,914 B gz over bare wgpu, against
RFC-001 §12 risk 10's 400,000 B cap. That leaves 43,086 B of headroom, and S8
alone added 37 KB (`crates/gup-core/PERF_BUDGETS.md`, History, last two
entries). S9 to S12 cannot fit at that rate, so a size story comes before S9.
This story builds the measuring tools. GUP-451 makes the cuts.

Nothing in the repository says where the bytes go. `scripts/wasm_size.sh`
reports three totals, and the per-story notes in `PERF_BUDGETS.md` were
estimated by building with and without a change. The story scribe parsed the
name and code sections of the scatter's cargo output by hand (before
wasm-bindgen, so it still includes `__wbindgen_describe_*` functions that
wasm-bindgen later strips). That parse gives the first picture, which this
story's tool must reproduce and replace:

- **`Selection<T, M>` is monomorphised per row type.** Its `Layer` methods take
  159,137 B of raw code, about 13% of the code section, spread over 7
  instantiations. The harness has six row types, all `Circle`. Per
  instantiation: `prepare` 8,177 B, `hit_items` 3,842 B (+737 B closure),
  `fit_domains` 3,712 B, `bind_axes` 1,404 B and `glue` 499 B. GUP-445 already
  noted that `hit_items` and `row_any` cost each new row type about 5.8 KB gz.
- **Fonts.** The Inter Regular and SemiBold subsets are 61,260 B and 62,308 B
  raw (30,204 B and 30,967 B gz on their own). SemiBold (GUP-436) is used only
  for major `Time` tick labels, which are years.
- **Other.** ttf-parser is about 64 KB raw (14 KB of it CFF parsing, which
  Inter's `glyf` outlines never use). The harness's async scene functions,
  inlined into `wasm_bindgen_futures` closures, are about 59 KB raw. wgpu itself
  is about 54 KB raw.

The owner also asked for a gate that makes per-instantiation growth visible: a
build with one more `Selection` row type, so that the cost of an additional `T`
is measured and budgeted. Today every new layer type a user writes pays for a
full copy of the `Layer` methods, and nothing reports it.

## Read First

- `crates/gup-core/PERF_BUDGETS.md` "Budget table" (lines 49–83), "WASM size"
  (lines 145–198) and the last two History entries (from "S8 so far").
- `scripts/wasm_size.sh` (all of it) and `maskfile.md` "wasm-size" (lines
  48–66).
- `scripts/perf_budget.sh`: how it reads the budget table's first three columns,
  and its `--wasm-only` mode.
- `crates/gup-core/wasm-size/scatter/Cargo.toml` and `src/lib.rs` lines 1–60 and
  760–923 (`show_scatter`, the `InteractiveScatter` hooks).
- `flake.nix` lines 70–118 (the dev shell's package list).
- RFC-001 "S3 findings → WASM size" (lines 1559–1600), for how twiggy was used
  on the naga_oil build.

## User Story

> "As a gup-core maintainer, I want one command that tells me which crates and
> functions the WASM bytes belong to, and a budget on what each extra row type
> costs, so that I can cut the right code and see generic bloat before it
> lands."

## Acceptance Criteria

### AC1: Spike — which attribution tool

- [ ] Run `nix shell nixpkgs#twiggy -c twiggy top -n 5 <wasm>` on the scatter as
      `wasm-bindgen --target web` writes it **with** its name section (no
      `--remove-name-section`). Record the twiggy version and the result in the
      story.
- [ ] Decision rule: if twiggy parses the file and prints function names (not
      only `code[N]`), the tool is twiggy, and `twiggy` joins the dev shell's
      packages in `flake.nix`. If it fails to parse (rustc enables
      reference-types and multi-value on wasm32 by default, and twiggy 0.7's
      parser predates them) or prints no names, the tool is a stdlib-only Python
      script, `scripts/wasm_attrib.py`, that reads the name, code and data
      sections itself (Decisions). Do not try a third tool.

### AC2: `mask wasm-attrib`

- [ ] `mask wasm-attrib` (`scripts/wasm_attrib.sh`, plus `wasm_attrib.py` if AC1
      chose it) builds the scatter harness exactly as `wasm_size.sh` does, binds
      it keeping names, and prints the three sections below, each headed by its
      name on a line of its own.
- [ ] Section `== top`: the 30 largest functions and data segments, raw bytes
      and percentage of the file.
- [ ] Section `== monos`: generic functions grouped by path with the
      instantiation hash removed, the 20 largest groups by total bytes, each
      with its instantiation count and total.
- [ ] Section `== crates`: raw bytes summed by crate (the first path segment of
      the demangled name, with a leading `<` stripped), with data segments as
      `data` and everything unmatched as `other`, largest first.
- [ ] It exits 0 on `main`. Its `== monos` section lists
      `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::prepare`
      with 7 instantiations. Its `== crates` section puts `gup_core` first.
- [ ] `maskfile.md` documents the task: what it builds, what each section means,
      that sizes are raw (find targets with it, judge cuts by the gz totals of
      `mask wasm-size`), and that it is local only, not a CI job.

### AC3: The per-row-type build

- [ ] The scatter harness gains a Cargo feature `extra-row-type`. With it, the
      harness exports one more function, `render_extra_row_type` (Decisions),
      which adds a `Selection` over a row type no other scene uses. Without the
      feature, the harness and its wasm are byte-identical to before (compare
      `sha256sum` of the bound `.wasm` before and after this story's harness
      edit, built without the feature).
- [ ] `scripts/wasm_size.sh` builds the scatter twice: with `extra-row-type`
      first and without it second, so the artifact left in
      `wasm32-unknown-unknown/release` is the default build that
      `wasm_browser.sh` uses. It prints a row `gup-core-scatter+T` and the
      metric `metric wasm.per_row_type.gz_bytes <with − without>`.
- [ ] `PERF_BUDGETS.md`'s budget table gains the row
      `wasm.per_row_type.gz_bytes | <measured> | info | scripts/wasm_size.sh, one more Selection row type`.
      `info` for now; GUP-451 sets its check after the cuts.
- [ ] `mask wasm-size` passes, and the `wasm.scatter` and `wasm.over_wgpu` rows
      are unchanged (the default build did not change).

### AC4: The attribution recorded

- [ ] `PERF_BUDGETS.md` "WASM size" gains a subsection "Where the bytes go
      (GUP-450)", with the date, the commit, and three tables from
      `mask wasm-attrib`: the 10 largest crates, the 10 largest generic groups,
      and the 10 largest single items. It adds one sentence on how far they
      agree with this story's Context figures, which came from the scribe's
      parse before wasm-bindgen.
- [ ] The same subsection records `wasm.per_row_type.gz_bytes` and the share of
      the scatter's gz bytes it represents.

## Decisions

- **The tool reads the bound wasm, keeping names.** It runs `wasm-bindgen`
  without `--remove-name-section`; that is the file a user ships, minus its
  names. Cargo's own output still holds wasm-bindgen's describe functions (about
  260 KB raw here), which would mislead.
- **Raw bytes for attribution, gz for judgement.** No attribution tool reports
  gzip contribution per function. Cuts are found with `mask wasm-attrib` and
  judged by `mask wasm-size`.
- **Fallback script, if AC1 needs it**: `scripts/wasm_attrib.py`, Python 3
  standard library only, run as `python3 -I`. It parses section headers; the
  import section (to count imported functions, which offset function indices);
  the code section (body sizes); subsection 1 of the `name` custom section
  (function names); and the data section (segment sizes). It demangles legacy
  Rust symbols by stripping `_ZN`, length prefixes and the `17h<16 hex>E` hash,
  and by replacing `$LT$ $GT$ $u20$ $C$ $RF$ $u7b$ $u7d$` and `..`. The `monos`
  grouping is the demangled name with the hash removed. It has a copyright
  header like the other scripts.
- **The extra row type** (`wasm-size/scatter/src/lib.rs`, behind
  `#[cfg(feature = "extra-row-type")]`):

  ```rust
  struct Extra { a: f64, b: f64, label: &'static str }

  #[wasm_bindgen]
  pub async fn render_extra_row_type(width: u32, height: u32) -> Result<Vec<u8>, JsValue>
  ```

  It creates a `Context`, adds 12 `Extra` rows through
  `plot.circles(rows).x(|r| r.a).y(|r| r.b)`, then
  `.tooltip(|r| r.label.to_owned())` and `.on(Event::Click, |_, _| {})`. It
  resolves at `width × height`, calls `plot.pick_row` at the plot rect's centre,
  renders through `ImageTarget` as `render_scatter` does, and returns the
  pixels. That reaches every `Layer` method an interactive layer uses:
  `bind_axes`, `fit_domains`, `prepare`, `hit_items`, `tooltip_text` and
  `dispatch`. The page never calls it; it is exported only so that it is linked.

- **A feature, not a second harness crate.** A second crate would need its own
  lockfile, CI cache key and wasm-bindgen pin. The cost is one extra build of
  the harness crate per `mask wasm-size` run (dependencies are shared).
- **Build order.** `extra-row-type` first, then the default build, so that
  `wasm_browser.sh`, which runs before `wasm_size.sh` in CI and after it
  locally, finds the default artifact.
- **No CI job for attribution.** It is a diagnostic. CI keeps the totals gate
  (GUP-438), plus the new metric as `info`.

## Escalation Triggers

- Without the feature, the bound wasm is not byte-identical to before (the
  harness edit changed the default build).
- `wasm.per_row_type.gz_bytes` is negative or above 20,000 B gz: the delta is
  not measuring what it should.
- `nix shell nixpkgs#twiggy` cannot run at all (no network or no package), so
  the spike cannot be done. Use the fallback script, and report this.

## Out of Scope

- Any cut (profile, fonts, de-monomorphisation): GUP-451.
- Changing the 400,000 B cap or the scatter's +2% check: GUP-451 re-records.
- A gate on a per-mark (`M`) instantiation: not requested; the harness has
  `Circle` layers only.

## Technical Tasks

- [ ] AC1 spike; record the result.
- [ ] `scripts/wasm_attrib.sh` (and `.py` if chosen) and `maskfile.md`
      `## wasm-attrib`; `twiggy` in `flake.nix` if chosen.
- [ ] Harness feature `extra-row-type` and `render_extra_row_type`.
- [ ] `wasm_size.sh`: second build, `gup-core-scatter+T` row and metric; update
      its header comment.
- [ ] `PERF_BUDGETS.md`: budget row and the "Where the bytes go" subsection.

## Dependencies

### Prerequisite Stories

- GUP-438: Trustworthy Goldens and Size Gates ✅ — `mask wasm-size` in CI and
  the `wasm.*` budget rows.
- GUP-449: RFC-001 S8f Web Canvas Host ✅ — the interactive harness being
  measured.

### Enables Stories

- GUP-451: WASM Size Cuts — finds its targets with `mask wasm-attrib` and
  budgets the per-row-type metric.

## Testing Strategy

- **Commands**: `mask wasm-attrib` (exit 0, the three sections, AC2's content),
  `mask wasm-size` (pass, new metric), `mask wasm-browser` (still passes: the
  default artifact is intact).
- **Byte identity**: `sha256sum` of the bound wasm without the feature, before
  and after the harness edit.
- **Python fallback, if chosen**: `python3 -I scripts/wasm_attrib.py` on the
  bound wasm prints a `crates` total within 1% of the file's code-plus-data
  size.

## Success Metrics

- [ ] One command attributes every raw byte of the scatter to a crate, function
      or data segment.
- [ ] The marginal cost of a row type is a number in `PERF_BUDGETS.md`, printed
      by every `mask wasm-size` run.

## Risk Assessment

- **Medium**: twiggy 0.7 may not parse current rustc wasm output. _Mitigation_:
  AC1's decision rule; the scribe's parse shows that the fallback works on this
  binary.
- **Low**: the second build adds time to CI's browser job (one more harness
  link). _Mitigation_: dependencies are cached and shared; note the time in the
  retrospective.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] `mask wasm-size`, `mask wasm-attrib` and `mask wasm-browser` pass
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
