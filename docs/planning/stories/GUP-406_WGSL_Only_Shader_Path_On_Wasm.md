# GUP-406: Build-Time Shader Composition

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-09)
**Created**: 2026-10-06 (rewritten 2026-10-09)

## Context

[GUP-401](GUP-401_RFC_001_S3_Scene_Renderer_RenderTarget.md) (RFC-001 S3)
measured gup-core's WASM cost against the RFC-001 §12 risk 10 budget of ≤ +400
KB gzipped for naga_oil. A bare-wgpu harness is 41.7 KB gz; the same harness
with its shader composed through naga_oil is 938.5 KB gz, so naga_oil costs
**+896.7 KB gz**, 2.2× the budget (`mask wasm-size`; RFC-001 "S3 findings"). The
cost is naga's WGSL front end, validator and WGSL back end (wgpu's WebGPU
backend turns `ShaderSource::Naga` back into WGSL for the browser), plus
naga_oil and the regex crates its preprocessor uses.

RFC-001 §6 and §12 risk 1 kept a fallback for exactly this: library modules use
only `#define_import_path`/`#import`, so they can be concatenated instead of
composed by naga_oil. The original version of this story planned to use that
fallback on wasm only, keeping naga_oil at runtime on native. The owner rejected
that design on 2026-10-09 (RFC-001 "Decisions (2026-10-09)"): a wasm-only
fallback is a second composition path living next to the first, and this
project's history is that parallel systems never get deleted on their own — four
scale systems, three composition systems, three pipeline caches, per the RFC-001
Orchestrator review's "parallel-system risk". The owner chose build-time
composition **on every target** instead: naga_oil runs once, at build time, to
flatten library modules into plain WGSL; at runtime, on native and wasm alike,
only the typed glue module is built and concatenated with that pre-flattened
text. naga_oil and naga become build-time-only dependencies of `gup-core`, never
linked into either binary.

gup-core also reads uniform member offsets from naga's layout of the composed
module (`struct_layout`). GUP-401 made every uniform `Params` struct span a
multiple of 16 bytes, so the `Encodings` offsets follow from encase sizes alone;
naga's layouter is not needed for that at runtime either, on any target.

S0a finding 8 (RFC-001) notes that the library's `as` aliases and `{Item}`
imports need a small qualification rewrite, not just concatenation, to produce
valid standalone WGSL — the same rewrite this story's concatenating composer
must do, just run once at build time rather than conditionally at runtime.

## User Story

> "As a Gup contributor, I want shader composition to run once, at build time,
> on every target, so that there is one composition path to maintain and the
> wasm build does not carry a shader compiler the browser already has."
>
> "As a web developer embedding a Gup chart, I want gup-core's wasm build not to
> carry naga and naga_oil, so that a chart costs a few hundred KB gzipped rather
> than over a megabyte."

## Acceptance Criteria

### AC1: naga_oil and naga are build-time-only

- [x] naga_oil and naga are not runtime dependencies of `gup-core` on any
      target.
      `cargo tree -p gup-core --target x86_64-unknown-linux-gnu -e normal` and
      `cargo tree -p gup-core --target wasm32-unknown-unknown -e normal` both
      omit `naga` and `naga_oil`.
- [x] naga_oil and naga are dependencies of the build-time tool only (a
      `[build-dependencies]` entry, a `build.rs`, or a separate build-time crate
      gup-core depends on at build time). wgpu's own transitive naga dependency
      (used for its internal validation at pipeline creation) is unaffected and
      out of scope.
- [x] naga as a `[dev-dependencies]` entry, for tests that re-parse composed
      WGSL (for example the S3 uniform-layout re-parse test), is permitted and
      does not count against this AC.

### AC2: library modules are flattened at build time; glue is built at runtime on every target

- [x] A build-time step (build script or build-time crate) runs naga_oil once
      over gup-core's library WGSL modules (§6: marks, scales, palettes, the
      view transform, colour helpers) and emits namespaced, mangled, plain WGSL
      text with no remaining `#import`/`#define_import_path` directives.
- [ ] `#[wgsl_function]` user modules (RFC-001 §4, §6) are flattened the same
      way, at macro-expansion time, not at runtime. **Deferred to S5:** gup-core
      has no `#[wgsl_function]` yet (it is RFC-001 S5). The old crate's
      `#[wgsl_function]` (gup-macros, GUP-006) is on the frozen path and has no
      naga step, so there is no expansion-time validation to extend. The
      flattening S5 needs is `gup_wgsl::compose::Library::new`; the RFC-001
      "GUP-406 findings" list what S5 must settle (import path, library sources
      at expansion, error spans, the calling convention).
- [x] At runtime, the existing typed glue emitter (§6) builds its one top-level
      module as a Rust string, unchanged in logic from today, and that string is
      concatenated with the pre-flattened library text. The result is passed to
      wgpu as `ShaderSource::Wgsl` on every target — one code path, not one per
      target.
- [x] The generated glue for the reference scatter
      (`crates/gup-core/tests/fixtures/scatter_glue.wgsl`) is byte-identical to
      its current checked-in fixture: the glue emitter's output and behaviour
      are unchanged by this story. `tests/golden/gup_core/scatter.png` and
      `window_parity` both still pass unmodified.

### AC3: composition errors still point at source modules at build time

- [x] A deliberately broken library module (for example a wrong-arity call, as
      in RFC-001 "S0a findings") produces a build-time error (`cargo build`
      fails) naming the source module and line, not a runtime panic. The story's
      evidence includes a sample error message.
- [ ] A deliberately broken `#[wgsl_function]` module produces a compile-time
      error at macro expansion (unchanged from today's behaviour), with a sample
      shown in the story's evidence. **Deferred to S5**, for the reason above.
      The library-module case (the first bullet) shows the report S5 will
      surface through `compile_error!`.

### AC4: authoring rules enforced at build time

- [x] The 16-byte `Params` rule (RFC-001 "S3 findings": every uniform `Params`
      struct spans a multiple of 16 bytes) is checked at build time, failing the
      build with a clear message naming the offending struct if violated.
- [x] The no-trailing-digit identifier rule (RFC-001 "S0a findings": naga_oil
      rejects identifiers like `r0` in composable modules) is checked at build
      time (naga_oil already enforces this when flattening; confirm the error
      surfaces at build time, not swallowed).
- [x] Uniform offsets used at runtime (native and wasm) come from encase sizes
      under the 16-byte rule, not from naga's layout of a composed module. A
      native test checks that these offsets equal naga's own layout of the fully
      composed reference glue, so the two never silently drift.

### AC5: WASM size budget

- [x] `mask wasm-size` reports the gup-core reference scatter within the ≤ +400
      KB gz budget over the bare-wgpu baseline (RFC-001 §12 risk 10), with the
      bundled Inter font reported as a separate line item (not counted against
      the +400 KB, per RFC-001 "S3 findings" precedent).
- [x] The number is recorded in RFC-001 (a short note near risk 10 or in a
      follow-up findings section), replacing the +897 KB gz figure.

### AC6: WASM default backends and browser behaviour

- [x] `ContextOptions::default()`'s wasm backends become
      `wgpu::Backends::BROWSER_WEBGPU` (dropping `GL`, per RFC-001 "Decisions
      (2026-10-09)"). wgpu's `webgl` feature remains disabled.
- [x] `mask wasm-browser` passes: the reference scatter renders correctly in
      headless Chromium over WebGPU, and the PNG is checked by eye and described
      in the retrospective.

### AC7: performance

- [x] Pipeline creation time (compose + create, release build) is no worse than
      RFC-001 "S0a findings" numbers (≤ 2.75 ms median, ≤ 13.5 ms cold) for the
      reference scatter, on native. Concatenation at runtime should be cheaper
      than today's full naga_oil composition, not more expensive; a regression
      here is a signal something is wrong.

## Technical Tasks

- [x] Design the build-time flattening step: a build script in `gup-core`, or a
      small build-time-only crate it depends on via `[build-dependencies]`, that
      runs naga_oil over the library modules and writes flattened, namespaced,
      mangled WGSL (one file, or one per module) into `$OUT_DIR`, included at
      compile time via `include_str!`.
- [x] Implement the qualification rewrite naga_oil's mangling already does
      (prefixing/mangling item names so flattened modules don't collide) — this
      is naga_oil's existing behaviour, just invoked at build time instead of
      per-`Context` at runtime.
- [ ] Move `#[wgsl_function]`'s existing compile-time naga validation (already
      at macro expansion) to also flatten the user's module the same way, so its
      output composes with the pre-flattened library by concatenation.
      **Deferred to S5** (no such macro in gup-core yet; see AC2).
- [x] Replace the runtime `Composer`/`Context.shaders` naga_oil state (RFC-001
      §2) with: the pre-flattened library text (a `&'static str` or similar,
      embedded at compile time) plus the existing glue emitter. Concatenate and
      pass `ShaderSource::Wgsl` to wgpu.
- [x] Add a build-time (or build-script-invoked) check for the 16-byte `Params`
      rule and the no-trailing-digit identifier rule, with error messages naming
      the offending module and struct/identifier.
- [x] Derive `Encodings`/`Chunk` uniform offsets from encase sizes on every
      target; keep the native cross-check test against naga's layout of the
      fully composed glue.
- [x] Remove naga_oil, naga and (if unused elsewhere) the `naga-ir` wgpu feature
      from `gup-core`'s runtime `[dependencies]`; add them under
      `[build-dependencies]` (or to the build-time crate) and, where needed for
      tests, `[dev-dependencies]`.
- [x] Change `ContextOptions::default()`'s wasm backends to
      `Backends::BROWSER_WEBGPU`.
- [x] Re-run `mask wasm-size` and `mask wasm-browser`; update RFC-001 with the
      new number and this story's outcome.

## Dependencies

### Prerequisite Stories

- GUP-401 ✅ — the size harnesses, `mask wasm-browser`, the 16-byte `Params`
  rule and the uniform-layout re-parse test this story builds on.

### Enables Stories

- RFC-001 S5 (`ShaderFn` v2, `#[wgsl_function]`) — enforces the 16-byte `Params`
  rule and the no-trailing-digit identifier rule at macro expansion, consistent
  with this story's build-time checks.
- RFC-001 S8 (wasm entry point) — at an acceptable download size, with one
  composition path to reason about.

## Testing Strategy

- **Unit tests**: the build-time flattening step produces WGSL naga itself (used
  only in tests/dev-dependencies) accepts; the 16-byte and no-trailing-digit
  checks fail on seeded violations with the expected error text.
- **Integration tests**: a native test composes the reference glue through the
  new runtime concatenation path and through naga_oil directly, and compares
  entry points and uniform struct layout — this is the test that proves native
  and wasm agree, since native is the only place naga_oil remains available to
  check against. `window_parity` and the existing golden-image test continue to
  pass unmodified.
- **Visual validation**: `mask wasm-browser`'s PNG is read by eye; the
  retrospective records what was seen.
- **Performance**: the pipeline-creation timing harness from RFC-001 "S0a
  findings" (`pipeline_timings`) is re-run and compared against its numbers.

## Success Metrics

- [x] `cargo tree -p gup-core --target wasm32-unknown-unknown -e normal` has no
      `naga`/`naga_oil` entry.
- [x] `mask wasm-size` for the reference scatter is within the ≤ +400 KB gz
      budget (from 896.7 KB gz over).
- [x] `mask wasm-browser` passes with an unchanged, correct render.

## Risk Assessment

- **Medium — unknown whether naga_oil's flattened output round-trips cleanly as
  plain WGSL.** RFC-001 "S3 findings" found that naga's own WGSL _writer_ (used
  when wgpu hands a browser `ShaderSource::Naga`) drops `@align(16)` attributes,
  which broke uniform layout in Chrome. This story's build-time step uses
  naga_oil's composition output directly (its own mangled WGSL text), not naga's
  writer, so that specific bug should not recur — but this needs verifying, not
  assuming. If naga_oil's flattened text also loses attributes or otherwise
  fails to parse standalone, the qualification rewrite needs more work than a
  simple prefix pass. Mitigation: the AC2/AC3 native tests compare the
  flattened-and-concatenated path's entry points and layout against naga_oil's
  direct composition before trusting it on wasm.
- **Medium — how runtime glue references mangled names.** The glue emitter
  currently emits `#import` paths and lets naga*oil resolve and mangle them.
  Once the library is pre-flattened, the glue must reference the \_mangled*
  names directly (or the build step must leave stable, documented item names the
  glue can target). Getting this wrong fails at pipeline creation, not at build
  time, since the glue itself is still built and validated at runtime by wgpu
  only. Mitigation: prefer a flattening scheme that preserves stable,
  predictable names (for example `gup__scale__linear__map_rel` rather than
  naga_oil's own hash-based mangling) so the glue emitter can target them
  directly without depending on naga_oil's internal mangling scheme, which is
  not a public contract.
- **Low — error message quality at build time vs today's runtime errors.**
  Build-time errors from a build script are seen once, at `cargo build`, and may
  be less visible than a runtime panic during development. Mitigation: fail the
  build loudly (non-zero exit, full naga_oil error text to stderr) and keep the
  native composition-equivalence test in CI so a broken library module is caught
  on every push, not just when someone happens to touch shader code.
- **Low — build script cost.** Running naga_oil at every `cargo build` (not
  cached across clean builds) adds build time. Mitigation: gate the flattening
  step on the library module files' mtimes (`cargo`'s normal build-script re-run
  detection via `println!("cargo:rerun-if-changed=...")`) so incremental builds
  are unaffected.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked, except the two
      `#[wgsl_function]` items, which are deferred to S5 (see AC2 and AC3)
- [x] `cargo test -p gup-core -- --test-threads=1` passes
- [x] `mask all-fix` is clean
- [x] All examples compile: `cargo check --examples`
- [x] The browser render was looked at and is described in the retrospective
- [x] Story status is updated in the story file and INDEX.md
- [x] A retrospective is added, including the new `mask wasm-size` numbers and a
      sample build-time error message

## Spike Findings (2026-10-09)

The two Risk Assessment unknowns were spiked first, in a new crate
`crates/gup-wgsl` (a throwaway `tests/spike.rs`, over gup-core's real library
and the checked-in reference glue; it became `crates/gup-wgsl/tests/compose.rs`
and the gup-core tests listed below).

### Round trip: works, with naga's writer and a build-time check

- naga_oil produces a `naga::Module`, not text, so printing flattened WGSL needs
  naga's WGSL writer: the same writer wgpu's WebGPU backend already used for
  every gup-core shader in the browser (S3). It is not a new risk, but the
  `@align` loss is real, so the build checks every round trip: each module's
  flattened text, with its imports', must parse and validate standalone, and
  every struct must lay out identically (span, member offsets and member names)
  to naga_oil's composed module.
- The reference glue, linked at run time against the flattened library, parses
  and validates as standalone WGSL. Its entry points and all 9 struct layouts
  (`Encodings` 64 bytes, `Chunk`, `Columns`, the three `Params`, `View`,
  `CircleIn`, `Varyings`) equal naga_oil's direct composition of the same glue.
- The check found one real hazard at once: naga's writer renames identifiers
  that end in a digit, so `rule.wgsl`'s `RuleIn.p0`/`p1` came back as
  `p0_`/`p1_`. Harmless there (vertex inputs bind by location), but a uniform
  struct member renamed that way would silently stop matching any by-name
  lookup. The members are now `start`/`stop` and the build rejects renamed
  members with a message saying why.
- naga_oil imports only items a shader names, so a probe that just `#import`s a
  module yields an empty module. The flattening probe passes every item through
  `NagaModuleDescriptor::additional_imports` instead (items come from a lexical
  scan of the module's declarations; a decorated name left over after renaming
  fails the build).
- naga's writer adds `@interpolate(flat)` to integer vertex _inputs_
  (`GradientIn.vertical`/`reverse`). It did so before this story too (wgpu wrote
  the same text for browsers), but no browser test drew a gradient. The browser
  harness now draws one, and Chrome accepts it.

### Mangling: stable, readable flat names

- Items are renamed in the IR from naga*oil's decorated names
  (`ParamsX_naga_oil_mod_XM52XAOR2ONRWC3DFHI5GY2LOMVQXEX`) to
  `gup_wgsl::flat_name(path, item)`: `::` →
  `*`, then the item (`gup_scale_linear_Params`, `gup_marks_circle_vertex`). These survive naga's namer unchanged (no `\_\_`,
  no trailing digit; the build fails if one does not, or if two items share a
  flat name). The scheme is gup-wgsl's own contract, not naga_oil's internal
  mangling.
- The run-time linker (`gup_wgsl::link`, no dependencies) reads the glue's
  naga_oil-style `#import` lines, rewrites `alias::item`, `full::path::item` and
  `{Item}` imports to flat names (comments and `.member` accesses untouched),
  comments out the `#import` lines so line numbers match the glue, and appends
  the flattened text of the imported modules and their imports once each. The
  glue emitter is unchanged.

### Errors at build time

```text
shader composition failed for src/shaders/broken.wgsl:
error: failed to build a valid final module: Function [1] 'gup::broken::f' is invalid
  ┌─ src/shaders/broken.wgsl:4:1
  │
4 │ ╭ fn f(view: gup::view::View) -> vec4<f32> {
5 │ │     return gup::view::px_to_clip(vec2<f32>(0.0));
  │ │            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ invalid function call
  │ ╰──────────────────────────────────────────────────^ naga::ir::Function [1]
  │
  = Call to [0] is invalid
  = Requires 2 arguments, but 1 are provided
```

- A `Params` member named `r0` fails in naga_oil ("Composable module identifiers
  must not require substitution according to naga writeback rules: `r0`",
  pointing at the struct). naga_oil checks only exported items and struct
  members; a trailing-digit function argument is renamed by the writer, which is
  harmless.
- A 4-byte `Params` fails: "struct `Params` of `gup::scale::pad` spans 4 bytes;
  uniform `Params` structs must span a multiple of 16 bytes (pad it in WGSL and
  in its encase twin)".

## Implementation Summary

Shader composition now runs once, at build time, on every target. naga_oil and
naga are dependencies of `gup-wgsl`'s build-time `compose` feature only; at run
time gup-core links its generated glue to pre-flattened WGSL and hands wgpu
`ShaderSource::Wgsl`, on native and wasm alike. The full record is in RFC-001
"GUP-406 findings".

### Key files

- `crates/gup-wgsl/` (new): `src/lib.rs` (`WgslModule`, `flat_name`, the
  dependency-free `link`), `src/lex.rs` (token scanner and declaration
  splitter), `src/compose.rs` (feature `compose`: `Library`, `read_dir`,
  `flatten_shader`, the round-trip and authoring-rule checks),
  `tests/compose.rs`.
- `crates/gup-core/build.rs` (new): flattens `src/shaders` into
  `$OUT_DIR/shaders.rs` and `$OUT_DIR/wgsl/*.wgsl`; fails the build with
  naga_oil's report.
- `crates/gup-core/src/shader/mod.rs`: the runtime `ShaderLibrary`/naga_oil
  composer is gone; generated statics, `link`, `StructLayout`.
- `crates/gup-core/src/shader/glue.rs`: `Glue` carries `Encodings`/`Chunk`
  layouts computed from encase sizes; the emitted text is unchanged.
- `crates/gup-core/src/render.rs`, `context.rs`: programs hold linked WGSL; the
  `shaders` mutex and its lock rank are gone; `PipelineStats` reports
  `programs_linked`/`last_link`; wasm backends default to `BROWSER_WEBGPU`.
- `crates/gup-core/Cargo.toml`: naga, naga_oil and wgpu's `naga-ir` leave
  `[dependencies]`; `gup-wgsl` is a normal dependency (no features), a
  build-dependency (`compose`) and a dev-dependency (`compose`, the test
  oracle), with naga as a dev-dependency.
- `crates/gup-core/src/shaders/rule.wgsl`: `p0`/`p1` → `start`/`stop` (naga's
  writer renamed them).
- `crates/gup-core/tests/fixtures/scatter_linked.wgsl` replaces
  `scatter_composed.wgsl`; `scatter_glue.wgsl` is unchanged.
- `crates/gup-core/wasm-size/`: the baseline loses its `naga-oil` feature; the
  scatter harness draws a background and legend; `maskfile.md` `wasm-size`
  prints gup-core's cost over wgpu.

### Tests and evidence

- gup-wgsl: 6 unit and 11 integration tests (round trip against naga_oil, seeded
  wrong arity, trailing-digit member, short `Params`, lost `@align`, renamed
  top-level member, `#ifdef`, flat-name collision, unknown import, two modules
  with one path).
- gup-core: 53 library tests plus every integration test pass, 2 ignored by
  design (`pipeline_timings`, `window_parity`, both run separately). New:
  `reference_glue_matches_fixtures`, `linked_glue_matches_naga_oil_composition`,
  `uniform_offsets_match_naga_layout`,
  `every_library_module_is_standalone_wgsl`, `guide_shaders_are_complete_wgsl`,
  `link_errors_name_the_generated_line`.
- Goldens byte-identical: `cargo test -p gup-core` (scatter, MSAA 1×/4×, scene
  items with the gradient and rect pipelines, SVG) and `-p gup-text` pass with
  no golden changes; `window_parity --ignored` passes; `mask visual-regression`
  (old path) passes 16/16.
- `cargo tree -p gup-core -e normal`: no naga_oil on either target, no naga on
  wasm32; on native naga only under wgpu-core/wgpu-hal.
- `mask wasm-size`: 392.9 KB gz (was 1,241.9), +351.1 KB over bare wgpu with
  Inter, +152.9 KB without.
- `pipeline_timings` (release): link + create 0.93 ms median, 8.4 ms cold (S0a:
  2.75 and 13.5).
- `mask wasm-browser`: PASS; the PNG is described in the retrospective.
- `mask all-check` clean; `cargo check --examples` clean; `mask old-path-loc`
  28935 (unchanged).
