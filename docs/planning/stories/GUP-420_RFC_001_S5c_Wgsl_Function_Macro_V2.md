# GUP-420: RFC-001 Step S5c: `#[wgsl_function]` v2 on Build-Time Composition

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-10

## Context

[RFC-001](../rfcs/RFC-001_Core_Architecture.md) §6 says `#[wgsl_function]` "now
emits a `ShaderFn` impl plus a module whose import path comes from
`module_path!()`. It validates the WGSL with naga **at macro expansion**, so
errors appear at compile time in external crates instead of as a runtime panic."
[GUP-406](GUP-406_WGSL_Only_Shader_Path_On_Wasm.md) moved shader composition to
build time on every target and explicitly deferred this macro: its findings list
"The two `#[wgsl_function]` items are deferred to S5 (no such macro in gup-core
yet)" in the INDEX, and spell out four open items under "S5
(`#[wgsl_function]`)" in RFC-001's "GUP-406 findings" section:

1. **The import path.** "A proc macro cannot evaluate `module_path!()`, and flat
   names need the path at expansion. Default to `<CARGO_CRATE_NAME>::<fn name>`,
   with a `path = "…"` override. `link` already refuses two different modules
   with one import path, so a collision fails at link time with a clear message,
   not inside wgpu."
2. **Library sources at expansion.** "A user module that imports `gup::view`
   needs that module's source to compose against. Move `src/shaders` (or its
   library half) into a crate both `gup-core/build.rs` and the macro can
   `include_str!` from: `gup-wgsl` itself, or a small `gup-shaders` data crate."
3. **Error spans.** "`compose::Error` reports point into the WGSL string. Emit
   them with `compile_error!` at the attribute's span, keeping naga_oil's report
   text."
4. **The calling convention.** "Check the entry function's signature against
   what the glue calls (`(In, [base: f32,] Params[, texture, sampler]) -> Out`)
   at expansion. naga_oil used to catch a mismatch when composing the glue at
   run time; now it would surface only as a wgpu validation error at pipeline
   creation."

[GUP-410](GUP-410_Surface_WebGPU_Errors_From_Gup_Core.md)'s findings add a fifth
requirement from the error-scope work: "a user function that composes at
expansion but fails pipeline creation... is an `Error::Gpu` naming the mark
pipeline and its glue signature. Keep the user module's import path in the glue
signature, so that the message names the user's function."

This story is **not** the old path's `#[wgsl_function]` macro
(`gup-macros/src/wgsl_function.rs`), which parses a Rust-syntax function body
and transpiles it to WGSL text for the frozen old render path. That macro is
untouched — per the Orchestrator review's "no feature work on the old path"
rule, and because it solves a different problem (Rust→WGSL transpilation, which
RFC-001 §1 explicitly rules out as a non-goal: "A Rust-to-WGSL transpiler. T1
deletes it. CPU mirrors are hand-written and tested against the GPU."). This
story's macro validates **authored WGSL** (the user writes the WGSL function
body directly, as every library scale module already does under
`crates/gup-core/src/shaders`) and emits the Rust `ShaderFn` glue around it,
following exactly the pattern `gup-core/build.rs` already proves for library
modules, but at macro-expansion time for a user's own crate.
[RFC-001's S0a findings](../rfcs/RFC-001_Core_Architecture.md) and
["GUP-406 findings"](../rfcs/RFC-001_Core_Architecture.md) record the authoring
rules this macro must enforce (no trailing-digit identifiers, every `Params`
struct spanning a multiple of 16 bytes) at expansion, alongside the calling-
convention check above — the same rules `build.rs` enforces for the library, now
enforced for anyone outside `gup-core` writing their own `ShaderFn`.

This story depends on
[GUP-418](GUP-418_RFC_001_S5a_ShaderFn_V2_And_Numeric_Scales.md) (S5a) settling
the final shape of the `ShaderFn` trait (in particular, whether a user-authored
function can participate in a `then` chain, and what
`params`/`chunk_base`/`fit_domain`/`resources` require), since this macro's
whole job is to emit an impl of that trait.

## User Story

> "As a visualization developer, I want to write my own GPU scale or colour
> function as plain WGSL in my own crate, with `#[wgsl_function]` generating the
> `ShaderFn` impl and catching a broken calling convention, a reserved
> identifier, or a WGSL syntax error at compile time with a message that names
> my function, so that I can extend Gup's scale family without learning
> gup-core's internal composition machinery or debugging a runtime panic."

## Acceptance Criteria

### AC1: Default and overridable import path

- [ ] A function annotated `#[wgsl_function]` in an external crate gets a
      default import path of `<CARGO_CRATE_NAME>::<fn name>` (matching the RFC's
      recommendation, since `module_path!()` is unavailable to a proc macro).
- [ ] `#[wgsl_function(path = "my::custom::path")]` (or equivalent attribute
      syntax) overrides the default.
- [ ] Two macro-generated modules that collide on the same import path (one user
      crate importing another's `#[wgsl_function]` output under a clashing path,
      or two functions in one crate with the same override) fail at
      `gup_wgsl::link` time with a clear message naming both paths, not inside
      wgpu — proved by a test.

### AC2: Library sources reachable at macro expansion

- [ ] A user's `#[wgsl_function]` body that imports a Gup library module (for
      example `gup::view` or a scale module) composes successfully at macro
      expansion, because the macro can read that library's WGSL source at
      expansion time.
- [ ] The library WGSL sources are reachable from both `gup-core/build.rs` and
      this macro without duplicating the files (a shared crate — `gup-     wgsl`
      itself or a new small data crate — is the single source of truth);
      document which, and why.
- [ ] A test in an external-crate-style integration test (not inside `gup-core`
      itself) proves a `#[wgsl_function]` importing a Gup library module
      compiles and runs correctly.

### AC3: Compile-time error reporting

- [ ] A `#[wgsl_function]` body with a WGSL syntax or composition error
      (reserved/trailing-digit identifier, undefined import, wrong-arity call)
      fails `cargo build`/`cargo check` with a `compile_error!` at the
      attribute's span, carrying naga_oil's own report text (source-mapped, as
      `build.rs`'s existing error path does for the library).
- [ ] A `trybuild` test captures this failure's message for at least two
      distinct error classes (a trailing-digit identifier; a wrong-arity call),
      matching the authoring rules RFC-001 already documents for library
      modules.
- [ ] Every `Params` struct the macro infers or requires spans a multiple of 16
      bytes, checked (and rejected with a clear compile-time message if not) at
      expansion — the same rule `build.rs` enforces for the library.

### AC4: Calling-convention check at expansion

- [ ] The macro checks the annotated function's signature against the shape the
      glue emitter calls:
      `(In, [base: f32,] Params[, texture, sampler])     -> Out`, for both
      absolute and `F32Relative`-style inputs, and for functions that take a LUT
      resource.
- [ ] A mismatched signature (wrong argument count, wrong argument order,
      missing `base: f32` for a relative-input function) fails at macro
      expansion with a message naming the expected signature, not merely at wgpu
      pipeline creation.
- [ ] A `trybuild` test proves this for at least one mismatch case.

### AC5: `Error::Gpu` names the user's function

- [ ] A `#[wgsl_function]`-generated module that composes successfully at
      expansion but fails wgpu's own validation at pipeline creation (for
      example, a browser-only uniformity error the macro cannot catch
      statically) surfaces as `Error::Gpu` naming the mark pipeline **and** the
      user's module's import path in the glue signature — not an opaque internal
      name.
- [ ] A test seeds such a failure (reusing the approach GUP-410's own tests use)
      and asserts the resulting `Error::Gpu` message contains the user's
      function's import path.

### AC6: External-crate proof and browser

- [ ] A doctest or external-crate-style integration test defines a
      `#[wgsl_function]`-based custom scale (or colour function) outside
      `gup-core`, uses it to encode a channel, and renders a chart whose output
      is checked through the `gup-visual-regression` harness's perceptual
      tolerance (never a byte-exact comparison to a PNG blessed only on this
      machine) — proving the public macro works end to end from outside the
      crate, which is the exact failure mode the strategic review's dogfood
      audit found in the old path ("`#[wgsl_function]` in an external crate
      panicked at runtime").
- [ ] `mask wasm-browser` renders a scene using a `#[wgsl_function]`-defined
      custom function and passes on WebGPU/SwiftShader.

### AC7: Old-path freeze

- [ ] `gup-macros/src/wgsl_function.rs` (the old path's macro) is unchanged by
      this story.
- [ ] No file outside `crates/gup-core`, the new macro crate this story adds,
      and planning docs changes; `mask old-path-loc` reports the same count as
      before this story.

## Technical Tasks

- [ ] Create a new proc-macro crate for this macro (for example
      `crates/     gup-core-macros`), separate from the frozen `gup-macros`, and
      add it as a dependency of `gup-core`.
- [ ] Implement the `#[wgsl_function]` attribute macro: parse the annotated
      function as WGSL (or as a thin wrapper identifying a WGSL source
      string/file), determine its import path (default and `path =` override),
      and flatten it via `gup-wgsl`'s `compose` feature at macro expansion
      (`Library::new`-style, per the GUP-406 findings).
- [ ] Move or share the library WGSL sources (`crates/gup-core/src/shaders` or a
      subset) into a crate reachable from both `gup-core/build.rs` and this new
      macro crate at expansion time.
- [ ] Implement the calling-convention check against the glue's call shapes
      (absolute, relative, LUT-resource variants).
- [ ] Route every macro-expansion error (naga_oil's report, the calling-
      convention mismatch, the 16-byte/identifier rule violations) through
      `compile_error!` at the attribute's span.
- [ ] Generate the `ShaderFn` impl (and `WgslModule` static) from the flattened
      WGSL, matching GUP-418's finalised trait shape.
- [ ] Add the `trybuild` suite for AC1, AC3 and AC4.
- [ ] Add the external-crate integration test/doctest and the seeded
      `Error::Gpu`-naming test (AC5, AC6).
- [ ] Extend `mask wasm-browser`'s harness with a `#[wgsl_function]`-defined
      scene (AC6).
- [ ] Re-run `mask old-path-loc` and confirm it is unchanged.

## Dependencies

### Prerequisite Stories

- GUP-418 (S5a) 📋 — finalises the `ShaderFn` trait shape (`params`,
  `chunk_base`, `fit_domain`, `resources`, and chain-compatibility via `then`)
  that this macro must emit an impl of.
- GUP-406 ✅ — `gup-wgsl`'s build-time `compose` feature and the flattening/
  linking machinery (`Library::new`, `link`, `flat_name`) this macro reuses at
  macro-expansion time instead of at a separate runtime step.
- GUP-410 ✅ — the `Error::Gpu` error-scope machinery whose message this story's
  AC5 extends to name a user's function.

### Enables Stories

- RFC-001 S10 and later builder-porting stories (not yet written) — a builder or
  an external user can add a custom encoding without reaching into `gup-core`'s
  internals, closing the gap the strategic review's dogfood audit found in the
  old path.
- The `docs/tutorials/03_custom_shader_functions.md` tutorial (currently
  describing the old path's macro) will need updating once this story lands and
  again at the RFC-001 S14 flip; not scheduled here.

## Testing Strategy

- **Compile-time tests**: `trybuild` for the import-path collision (AC1),
  WGSL/identifier errors (AC3) and calling-convention mismatches (AC4).
- **Unit/integration tests**: the shared library-source reachability proof
  (AC2), the seeded `Error::Gpu`-naming test (AC5).
- **External-crate/doctest**: AC6's end-to-end custom-scale proof.
- **Browser**: `mask wasm-browser` extended per AC6.

## Success Metrics

- [ ] A WGSL authoring mistake in a `#[wgsl_function]` body fails
      `cargo     build` with a message naming the function and the problem, not
      a runtime panic — closing the exact gap the strategic review's dogfood
      audit found ("`#[wgsl_function]` in an external crate panicked at
      runtime").
- [ ] A calling-convention mismatch is caught at macro expansion in every tested
      case, not only at wgpu pipeline creation.
- [ ] `mask wasm-browser` and `mask old-path-loc` are unaffected beyond the
      intended scene addition and the new macro crate.

## Risk Assessment

- **Medium**: sharing library WGSL sources between `gup-core/build.rs` and a
  separate proc-macro crate (AC2) may require restructuring where those `.wgsl`
  files live, which touches `build.rs`'s existing, already-proven generation
  path. _Mitigation_: keep `build.rs`'s own generated output and tests (the
  struct-layout and flattening checks from GUP-406) passing unchanged
  throughout; treat any required `build.rs` change as additive (a new shared
  location it reads from), not a rewrite.
- **Medium**: the calling-convention check (AC4) must match the glue emitter's
  call shapes exactly, including the shapes GUP-418 (S5a, `then` chains) and
  GUP-419 (S5b, dictionary-keyed and LUT-resource functions) may introduce or
  extend. _Mitigation_: depend on GUP-418 landing first (listed above) and
  re-verify the check's coverage against GUP-419's shapes before closing this
  story, even though GUP-419 is not a hard dependency.
- **Low**: proc-macro crates cannot easily share a `cfg(test)`-only test harness
  with the crate they generate code for, which may complicate the external-crate
  proof (AC6). _Mitigation_: use a separate integration-test crate or `tests/`
  directory (as `gup-core`'s existing `compile_fail`/ `compile_pass` directories
  already do) rather than inventing a new mechanism.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read) for AC6
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
