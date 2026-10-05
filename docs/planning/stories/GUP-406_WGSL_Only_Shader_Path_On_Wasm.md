# GUP-406: WGSL-Only Shader Path on Wasm

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-06

## Context

[GUP-401](GUP-401_RFC_001_S3_Scene_Renderer_RenderTarget.md) (RFC-001 S3)
measured gup-core's WASM cost against the RFC-001 §12 risk 10 budget of ≤ +400
KB gzipped for naga_oil. A bare-wgpu harness is 41.7 KB gz; the same harness
with its shader composed through naga_oil is 938.5 KB gz, so naga_oil costs
**+896.7 KB gz**, 2.2× the budget (`mask wasm-size`; RFC-001 "S3 findings"). The
cost is naga's WGSL front end, validator and WGSL back end (wgpu's WebGPU
backend turns `ShaderSource::Naga` back into WGSL), plus naga_oil and the regex
crates its preprocessor uses.

RFC-001 §6 and §12 risk 1 kept a fallback for exactly this: library modules use
only `#define_import_path`/`#import`, so they can be concatenated. S0a finding 8
notes that the `as` aliases and `{Item}` imports need a small qualification
rewrite, not just concatenation. In a browser, the browser parses WGSL itself;
naga is redundant there.

gup-core also reads uniform member offsets from naga's layout of the composed
module (`struct_layout`). GUP-401 made every uniform `Params` struct span a
multiple of 16 bytes, so the `Encodings` offsets follow from encase sizes alone.
naga is not needed for that either.

## User Story

> "As a web developer embedding a Gup chart, I want gup-core's wasm build not to
> carry a shader compiler the browser already has, so that a chart costs a few
> hundred KB gzipped rather than over a megabyte."

## Acceptance Criteria

- [ ] **Decision recorded in RFC-001**: on `wasm32`, gup-core composes WGSL by
      namespaced concatenation and passes `ShaderSource::Wgsl`; native keeps
      naga_oil (source-mapped errors, validation). Alternatives (for example
      precomposed shaders) are noted with the reason for rejecting them.
- [ ] A `Composer`-shaped internal seam (RFC-001 §12 risk 1) selects naga_oil on
      native and the concatenating composer on wasm. On wasm32, naga, naga_oil
      and regex are absent from
      `cargo tree -p gup-core --target     wasm32-unknown-unknown -e normal`.
- [ ] The concatenating composer produces WGSL that **native naga validates**,
      for every library module, the reference glue and the guide shaders. A
      native test composes each both ways and compares entry points and the
      layout of every uniform struct.
- [ ] Uniform offsets on wasm come from encase sizes under the 16-byte `Params`
      rule. A native test checks that they equal naga's layout for the reference
      glue.
- [ ] `mask wasm-size` reports the gup-core scatter at **≤ 450 KB gz** (wgpu
      ~42 + gup-core ~105 + Inter ~198 + margin; less after GUP-407). The number
      is recorded in RFC-001.
- [ ] `mask wasm-browser` passes: the scatter renders correctly in headless
      Chromium, and the PNG is checked by eye.

## Technical Tasks

- [ ] Add an internal composer seam in `shader/mod.rs` with naga_oil and
      concatenating implementations, chosen by `cfg(target_arch)`.
- [ ] Implement the concatenating composer: prefix each library module's items
      with its path, rewrite `alias::item` and `{Item}` imports, and emit
      modules in dependency order.
- [ ] Derive `Encodings`/`Chunk` offsets from encase sizes on wasm.
- [ ] Make naga and naga_oil native-only dependencies, and drop wgpu's `naga-ir`
      feature on wasm.
- [ ] Re-measure, and update RFC-001 and the risk 10 entry.

## Dependencies

### Prerequisite Stories

- GUP-401 ✅ — the size harnesses, `mask wasm-browser` and the 16-byte `Params`
  rule.

### Enables Stories

- RFC-001 S8 (wasm entry point) at an acceptable download size.

## Testing Strategy

- Native unit tests: both composers give equivalent, naga-valid modules.
- `mask wasm-size` for the number, and `mask wasm-browser` for behaviour.

## Success Metrics

- gup-core scatter ≤ 450 KB gz (from 1,241.9 KB).

## Risk Assessment

- **Medium**: error messages on wasm lose naga_oil's source mapping. Mitigation:
  the same glue is validated natively in CI, and browser errors still name the
  generated source.
- **Low**: name collisions from prefixing. Mitigation: the native equivalence
  test covers every module.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `cargo test -p gup-core -- --test-threads=1` passes
- [ ] `mask all-fix` is clean
- [ ] The browser render was looked at and is described in the retrospective
- [ ] Story status is updated in the story file and INDEX.md
- [ ] A retrospective is added
