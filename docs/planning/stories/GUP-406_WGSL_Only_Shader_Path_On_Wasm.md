# GUP-406: Build-Time Shader Composition

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-06 (rewritten 2026-10-09)

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

- [ ] naga_oil and naga are not runtime dependencies of `gup-core` on any
      target.
      `cargo tree -p gup-core --target x86_64-unknown-linux-gnu -e     normal`
      and `cargo tree -p gup-core --target wasm32-unknown-unknown -e     normal`
      both omit `naga` and `naga_oil`.
- [ ] naga_oil and naga are dependencies of the build-time tool only (a
      `[build-dependencies]` entry, a `build.rs`, or a separate build-time crate
      gup-core depends on at build time). wgpu's own transitive naga dependency
      (used for its internal validation at pipeline creation) is unaffected and
      out of scope.
- [ ] naga as a `[dev-dependencies]` entry, for tests that re-parse composed
      WGSL (for example the S3 uniform-layout re-parse test), is permitted and
      does not count against this AC.

### AC2: library modules are flattened at build time; glue is built at runtime on every target

- [ ] A build-time step (build script or build-time crate) runs naga_oil once
      over gup-core's library WGSL modules (§6: marks, scales, palettes, the
      view transform, colour helpers) and emits namespaced, mangled, plain WGSL
      text with no remaining `#import`/`#define_import_path` directives.
- [ ] `#[wgsl_function]` user modules (RFC-001 §4, §6) are flattened the same
      way, at macro-expansion time, not at runtime.
- [ ] At runtime, the existing typed glue emitter (§6) builds its one top-level
      module as a Rust string, unchanged in logic from today, and that string is
      concatenated with the pre-flattened library text. The result is passed to
      wgpu as `ShaderSource::Wgsl` on every target — one code path, not one per
      target.
- [ ] The generated glue for the reference scatter
      (`crates/gup-core/tests/     fixtures/scatter_glue.wgsl`) is
      byte-identical to its current checked-in fixture: the glue emitter's
      output and behaviour are unchanged by this story.
      `tests/golden/gup_core/scatter.png` and `window_parity` both still pass
      unmodified.

### AC3: composition errors still point at source modules at build time

- [ ] A deliberately broken library module (for example a wrong-arity call, as
      in RFC-001 "S0a findings") produces a build-time error (`cargo build`
      fails) naming the source module and line, not a runtime panic. The story's
      evidence includes a sample error message.
- [ ] A deliberately broken `#[wgsl_function]` module produces a compile-time
      error at macro expansion (unchanged from today's behaviour), with a sample
      shown in the story's evidence.

### AC4: authoring rules enforced at build time

- [ ] The 16-byte `Params` rule (RFC-001 "S3 findings": every uniform `Params`
      struct spans a multiple of 16 bytes) is checked at build time, failing the
      build with a clear message naming the offending struct if violated.
- [ ] The no-trailing-digit identifier rule (RFC-001 "S0a findings": naga_oil
      rejects identifiers like `r0` in composable modules) is checked at build
      time (naga_oil already enforces this when flattening; confirm the error
      surfaces at build time, not swallowed).
- [ ] Uniform offsets used at runtime (native and wasm) come from encase sizes
      under the 16-byte rule, not from naga's layout of a composed module. A
      native test checks that these offsets equal naga's own layout of the fully
      composed reference glue, so the two never silently drift.

### AC5: WASM size budget

- [ ] `mask wasm-size` reports the gup-core reference scatter within the ≤ +400
      KB gz budget over the bare-wgpu baseline (RFC-001 §12 risk 10), with the
      bundled Inter font reported as a separate line item (not counted against
      the +400 KB, per RFC-001 "S3 findings" precedent).
- [ ] The number is recorded in RFC-001 (a short note near risk 10 or in a
      follow-up findings section), replacing the +897 KB gz figure.

### AC6: WASM default backends and browser behaviour

- [ ] `ContextOptions::default()`'s wasm backends become
      `wgpu::Backends::BROWSER_WEBGPU` (dropping `GL`, per RFC-001 "Decisions
      (2026-10-09)"). wgpu's `webgl` feature remains disabled.
- [ ] `mask wasm-browser` passes: the reference scatter renders correctly in
      headless Chromium over WebGPU, and the PNG is checked by eye and described
      in the retrospective.

### AC7: performance

- [ ] Pipeline creation time (compose + create, release build) is no worse than
      RFC-001 "S0a findings" numbers (≤ 2.75 ms median, ≤ 13.5 ms cold) for the
      reference scatter, on native. Concatenation at runtime should be cheaper
      than today's full naga_oil composition, not more expensive; a regression
      here is a signal something is wrong.

## Technical Tasks

- [ ] Design the build-time flattening step: a build script in `gup-core`, or a
      small build-time-only crate it depends on via `[build-dependencies]`, that
      runs naga_oil over the library modules and writes flattened, namespaced,
      mangled WGSL (one file, or one per module) into `$OUT_DIR`, included at
      compile time via `include_str!`.
- [ ] Implement the qualification rewrite naga_oil's mangling already does
      (prefixing/mangling item names so flattened modules don't collide) — this
      is naga_oil's existing behaviour, just invoked at build time instead of
      per-`Context` at runtime.
- [ ] Move `#[wgsl_function]`'s existing compile-time naga validation (already
      at macro expansion) to also flatten the user's module the same way, so its
      output composes with the pre-flattened library by concatenation.
- [ ] Replace the runtime `Composer`/`Context.shaders` naga_oil state (RFC-001
      §2) with: the pre-flattened library text (a `&'static str` or similar,
      embedded at compile time) plus the existing glue emitter. Concatenate and
      pass `ShaderSource::Wgsl` to wgpu.
- [ ] Add a build-time (or build-script-invoked) check for the 16-byte `Params`
      rule and the no-trailing-digit identifier rule, with error messages naming
      the offending module and struct/identifier.
- [ ] Derive `Encodings`/`Chunk` uniform offsets from encase sizes on every
      target; keep the native cross-check test against naga's layout of the
      fully composed glue.
- [ ] Remove naga_oil, naga and (if unused elsewhere) the `naga-ir` wgpu feature
      from `gup-core`'s runtime `[dependencies]`; add them under
      `[build-dependencies]` (or to the build-time crate) and, where needed for
      tests, `[dev-dependencies]`.
- [ ] Change `ContextOptions::default()`'s wasm backends to
      `Backends::BROWSER_WEBGPU`.
- [ ] Re-run `mask wasm-size` and `mask wasm-browser`; update RFC-001 with the
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

- [ ] `cargo tree -p gup-core --target wasm32-unknown-unknown -e normal` has no
      `naga`/`naga_oil` entry.
- [ ] `mask wasm-size` for the reference scatter is within the ≤ +400 KB gz
      budget (from 896.7 KB gz over).
- [ ] `mask wasm-browser` passes with an unchanged, correct render.

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

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `cargo test -p gup-core -- --test-threads=1` passes
- [ ] `mask all-fix` is clean
- [ ] All examples compile: `cargo check --examples`
- [ ] The browser render was looked at and is described in the retrospective
- [ ] Story status is updated in the story file and INDEX.md
- [ ] A retrospective is added, including the new `mask wasm-size` numbers and a
      sample build-time error message
