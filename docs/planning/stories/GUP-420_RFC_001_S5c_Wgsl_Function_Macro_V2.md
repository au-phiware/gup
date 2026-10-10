# GUP-420: RFC-001 Step S5c: `#[wgsl_function]` v2 on Build-Time Composition

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-11)
**Created**: 2026-10-10

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

- [x] A function annotated `#[wgsl_function]` in an external crate gets a
      default import path of `<CARGO_CRATE_NAME>::<fn name>` (matching the RFC's
      recommendation, since `module_path!()` is unavailable to a proc macro).
- [x] `#[wgsl_function(path = "my::custom::path")]` (or equivalent attribute
      syntax) overrides the default.
- [x] Two macro-generated modules that collide on the same import path (one user
      crate importing another's `#[wgsl_function]` output under a clashing path,
      or two functions in one crate with the same override) fail at
      `gup_wgsl::link` time with a clear message naming both paths, not inside
      wgpu — proved by a test.

_Delivered differently:_ the default path is
`<CARGO_CRATE_NAME>::<type in snake_case>`, since the attribute goes on
`impl ShaderFn for T` and entry functions are usually all `map`. A clash fails
before `link`, when the layer prepares, as `Error::Compose` naming both Rust
types: the glue emitter aliases imports by path, so `link` would never see the
second module, and the second function would silently call the first one's WGSL
(`two_functions_with_one_import_path_are_refused`).

### AC2: Library sources reachable at macro expansion

- [x] A user's `#[wgsl_function]` body that imports a Gup library module (for
      example `gup::view` or a scale module) composes successfully at macro
      expansion, because the macro can read that library's WGSL source at
      expansion time.
- [x] The library WGSL sources are reachable from both `gup-core/build.rs` and
      this macro without duplicating the files (a shared crate — `gup-     wgsl`
      itself or a new small data crate — is the single source of truth);
      document which, and why.
- [x] A test in an external-crate-style integration test (not inside `gup-core`
      itself) proves a `#[wgsl_function]` importing a Gup library module
      compiles and runs correctly.

_Delivered:_ the library modules live in `crates/gup-wgsl/library`, listed by a
gup-wgsl build script (`gup_wgsl::compose::library()`): both consumers already
enable gup-wgsl's `compose` feature, and its run-time half does not carry the
text. The proof is `a_wgsl_file_importing_the_library` (a softplus importing
`gup::scale::symlog`, read with `file = "…"`).

### AC3: Compile-time error reporting

- [x] A `#[wgsl_function]` body with a WGSL syntax or composition error
      (reserved/trailing-digit identifier, undefined import, wrong-arity call)
      fails `cargo build`/`cargo check` with a `compile_error!` at the
      attribute's span, carrying naga_oil's own report text (source-mapped, as
      `build.rs`'s existing error path does for the library).
- [x] A `trybuild` test captures this failure's message for at least two
      distinct error classes (a trailing-digit identifier; a wrong-arity call),
      matching the authoring rules RFC-001 already documents for library
      modules.
- [x] Every `Params` struct the macro infers or requires spans a multiple of 16
      bytes, checked (and rejected with a clear compile-time message if not) at
      expansion — the same rule `build.rs` enforces for the library.

_Delivered differently:_ a user's `Params` is padded in the flattened WGSL
instead of rejected (the twin covers the padding with encase's `size`), so users
write no padding; the library keeps the strict rule, and the rejection, naming
the members to add, remains for the rare `Params` that cannot be padded.

### AC4: Calling-convention check at expansion

- [x] The macro checks the annotated function's signature against the shape the
      glue emitter calls:
      `(In, [base: f32,] Params[, texture, sampler])     -> Out`, for both
      absolute and `F32Relative`-style inputs, and for functions that take a LUT
      resource.
- [x] A mismatched signature (wrong argument count, wrong argument order,
      missing `base: f32` for a relative-input function) fails at macro
      expansion with a message naming the expected signature, not merely at wgpu
      pipeline creation.
- [x] A `trybuild` test proves this for at least one mismatch case.

_Note:_ the input format is inferred from the signature, so an `f32` value
without a `base` is an absolute input, not an error. The relative cases that can
be wrong are caught: a hi/lo value without its base (`hilo_without_base`), and a
`chunk_base` hook on a function without a base.

### AC5: `Error::Gpu` names the user's function

- [x] A `#[wgsl_function]`-generated module that composes successfully at
      expansion but fails wgpu's own validation at pipeline creation (for
      example, a browser-only uniformity error the macro cannot catch
      statically) surfaces as `Error::Gpu` naming the mark pipeline **and** the
      user's module's import path in the glue signature — not an opaque internal
      name.
- [x] A test seeds such a failure (reusing the approach GUP-410's own tests use)
      and asserts the resulting `Error::Gpu` message contains the user's
      function's import path.

_Delivered:_ the seeded failure uses GUP-410's seeds on a macro-generated
function (`scope::tests::a_gpu_error_names_the_user_function`). The browser-only
class the AC names is narrower than expected: the macro now also rejects at
compile time what naga can see (vertex-stage operations, bindings, overrides).

### AC6: External-crate proof and browser

- [x] A doctest or external-crate-style integration test defines a
      `#[wgsl_function]`-based custom scale (or colour function) outside
      `gup-core`, uses it to encode a channel, and renders a chart whose output
      is checked through the `gup-visual-regression` harness's perceptual
      tolerance (never a byte-exact comparison to a PNG blessed only on this
      machine) — proving the public macro works end to end from outside the
      crate, which is the exact failure mode the strategic review's dogfood
      audit found in the old path ("`#[wgsl_function]` in an external crate
      panicked at runtime").
- [x] `mask wasm-browser` renders a scene using a `#[wgsl_function]`-defined
      custom function and passes on WebGPU/SwiftShader.

### AC7: Old-path freeze

- [x] `gup-macros/src/wgsl_function.rs` (the old path's macro) is unchanged by
      this story.
- [x] No file outside `crates/gup-core`, the new macro crate this story adds,
      and planning docs changes; `mask old-path-loc` reports the same count as
      before this story.

_Note:_ gup-wgsl changed too (the library moved there, `add_module`), as AC2
allows, and the root `Cargo.toml` gained the workspace member. `gup-macros` is
untouched and `mask old-path-loc` is 28,910 before and after.

## Technical Tasks

- [x] Create a new proc-macro crate for this macro (for example
      `crates/     gup-core-macros`), separate from the frozen `gup-macros`, and
      add it as a dependency of `gup-core`.
- [x] Implement the `#[wgsl_function]` attribute macro: parse the annotated
      function as WGSL (or as a thin wrapper identifying a WGSL source
      string/file), determine its import path (default and `path =` override),
      and flatten it via `gup-wgsl`'s `compose` feature at macro expansion
      (`Library::new`-style, per the GUP-406 findings).
- [x] Move or share the library WGSL sources (`crates/gup-core/src/shaders` or a
      subset) into a crate reachable from both `gup-core/build.rs` and this new
      macro crate at expansion time.
- [x] Implement the calling-convention check against the glue's call shapes
      (absolute, relative, LUT-resource variants).
- [x] Route every macro-expansion error (naga_oil's report, the calling-
      convention mismatch, the 16-byte/identifier rule violations) through
      `compile_error!` at the attribute's span.
- [x] Generate the `ShaderFn` impl (and `WgslModule` static) from the flattened
      WGSL, matching GUP-418's finalised trait shape.
- [x] Add the `trybuild` suite for AC1, AC3 and AC4.
- [x] Add the external-crate integration test/doctest and the seeded
      `Error::Gpu`-naming test (AC5, AC6).
- [x] Extend `mask wasm-browser`'s harness with a `#[wgsl_function]`-defined
      scene (AC6).
- [x] Re-run `mask old-path-loc` and confirm it is unchanged.

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

- [x] A WGSL authoring mistake in a `#[wgsl_function]` body fails
      `cargo     build` with a message naming the function and the problem, not
      a runtime panic — closing the exact gap the strategic review's dogfood
      audit found ("`#[wgsl_function]` in an external crate panicked at
      runtime").
- [x] A calling-convention mismatch is caught at macro expansion in every tested
      case, not only at wgpu pipeline creation.
- [x] `mask wasm-browser` and `mask old-path-loc` are unaffected beyond the
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

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: gup-core (123 unit, 14 doctests, every integration test,
      both trybuild drivers), gup-wgsl and gup-core-macros, with
      `--test-threads=1`. The root crate's tests are left to CI, per the
      orchestrator's rule not to build them locally.
- [x] Lint and format clean: `mask all-check` (fmt, strict clippy on the whole
      workspace with default and all features, wasm32 clippy) passes; the hook
      ran its scoped checks on every commit
- [x] gup-core's examples compile (`cargo check -p gup-core --examples`); the
      root crate's are left to CI, as above
- [x] Rendered output verified by eye (golden image or PNG read) for AC6
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

A user writes a shader function as WGSL in their own crate and puts
`#[wgsl_function]` on `impl ShaderFn for T`. At compile time the macro composes
the WGSL against Gup's library and checks it, then completes the impl. Every
mistake it can see is a compile error at the user's code. RFC-001 "S5c findings"
records the design and the measurements.

The newcomer example, from `gup_core::wgsl_function`'s doctest:

```rust
#[derive(Clone)]
struct Energy;

#[wgsl_function(monotonic, wgsl = r#"
    struct Params { slope: f32, offset: f32 }

    fn energy(magnitude: f32, p: Params) -> f32 {
        return pow(10.0, p.slope * magnitude + p.offset);
    }
"#)]
impl ShaderFn for Energy {
    fn params(&self) -> EnergyParams {
        EnergyParams { slope: 1.5, offset: 4.8 }
    }
}

impl CpuMirror for Energy {
    fn eval(&self, magnitude: f64) -> f64 {
        10f64.powf(1.5 * magnitude + 4.8)
    }
}

let area = Energy.then(Pow::sqrt().domain(0.0, Energy.eval(7.8)).range(Px(0.0), Px(30.0)));
```

### What was built

- **`crates/gup-core-macros`** (new): the attribute. `lib.rs` parses the
  arguments (`wgsl`, `file`, `path`, `entry`, `monotonic`). It composes the
  module, checks the impl's items against the WGSL, generates the `Params` twin
  (encase impl via `encase_derive_impl`, rooted at gup-core's re-export) and the
  `WgslModule` static, and caches the composed library per thread. After an
  error it emits a placeholder impl. `convention.rs` checks the entry function
  against the glue's calling convention, the vertex stage, bindings and
  overrides, and maps `Params` members to Rust.
- **`crates/gup-wgsl`**: the library modules moved to `library/`, listed by a
  new `build.rs` (`compose::library()`). `Library::add_module` and
  `remove_module` serve user modules, and user `Params` are padded
  (`PAD_PREFIX`). A did-you-mean suggests the closest import, and the 16-byte
  error names the members to add. `validate_wgsl` returns naga's `ModuleInfo`.
- **`crates/gup-core`**: re-exports `wgsl_function` (in the prelude, with the
  user-facing documentation and doctest). Also new: `__private` for the
  expansion, `extern crate self as gup_core`, and `Color` as an encase
  `vec4<f32>` (`#[repr(C)]`, `Pod`), which removes `Categorical`'s `Rgba`.
  `CpuMirror` gains an `on_unimplemented` message, and `DynShaderFn` a
  `type_name`. The layer refuses two functions with one import path. `build.rs`
  reads the library from gup-wgsl.
- **Tests**: `tests/wgsl_function.rs` (5 tests, 2 goldens:
  `gup_core/wgsl_function_aftershocks`, `gup_core/wgsl_function_highlight`),
  `tests/wgsl_function_fail` (15 trybuild cases), `tests/wgsl/softplus.wgsl`,
  `scope::tests::a_gpu_error_names_the_user_function`, two gup-wgsl tests and
  three macro unit tests.
- **Browser**: a sixth `mask wasm-browser` scene of user functions, checked
  against their mirrors. `PERF_BUDGETS.md` re-records the WASM scatter.

### Evidence

- **Goldens**, read before blessing; both are 0 of 324,000 pixels apart on
  lavapipe (max ΔE 1.2 and 1.3).
  - `wgsl_function_aftershocks`: a map of a synthetic aftershock sequence along
    a 300 km fault. Each disc's area grows with the energy released (the user's
    `Energy`, then `Pow::sqrt()` above a 1.5 px floor). The M7.8 main shock is a
    36 px disc, and three large aftershocks and hundreds of small ones lie along
    the fault. Colour is depth on reversed viridis with a ramp legend (0–40 km),
    so the fault's westward dip reads as a gradient from green-yellow (shallow,
    trench side) to purple (deep).
  - `wgsl_function_highlight`: 120 countries by GDP per capita and population
    (log), the 22 Asian ones in vermilion and the rest, including those with no
    continent, in light grey. It is drawn by a user colour function over
    dictionary codes, which finds "Asia" in `fit_keys`.
- **Pixels**: every disc inside the plot (the plot insets by the chain's
  `image`), the main shock's edge at the mirror's radius, and every visible
  centre in its mirror's colour (highlight, softplus, browser).
- **Browser**: `GUP PASS … user=2461` on Chrome 154 / SwiftShader.
- **`mask perf-budget`**: run 1 failed only the cold link-and-create (10.8 ms,
  Mesa's shader cache cold after the rebuild). Run 2 passed every timing check
  at a 500 MHz GPU clock median: GPU pass 3.75 ms, CPU 0.63 ms, 88 uniform and 0
  column bytes a frame, link and create 0.94 ms median, cold 1.08 ms. Run 3
  passed everything, including WASM (300,913 B gz scatter, 259,201 B over bare
  wgpu), at a boosted 750 MHz.
- `mask old-path-loc`: 28,910 before and after.

## Retrospective

**Completed**: 2026-10-11

### Key Technical Learnings

#### The size assertion paid for itself on the first colour

- **Challenge**: a `Params` twin generated from WGSL must lay out exactly as
  naga does, and the existing `Rgba` twin suggested that a struct of four `f32`s
  matches a `vec4<f32>`.
- **Solution**: the macro emits a constant assertion that the twin's encase size
  equals naga's span. It fired on the first `{vec4, vec4, u32}` (36 against 48
  bytes): encase aligns a nested struct of `f32`s to 4. `Color` became an encase
  vector (`impl_vector!`), and `Rgba`, which only matched because it came first,
  went.
- **Pattern**: when generating a twin of another tool's layout, assert the
  layout in the generated code. A test of one example would have passed.

#### Spans on stable are good enough

- **Challenge**: `Literal::subspan` is nightly-only, so an error inside a WGSL
  string can only point at the whole literal.
- **Solution**: `Span::line`, `column` and `file` are stable since Rust 1.88.
  Padding the WGSL with whitespace up to the literal's position makes naga_oil's
  report name the Rust file and the exact WGSL line. The `compile_error!` sits
  on the literal's first character, so rustc does not underline the whole string
  as well.

#### A cached composer needs an undo

- **Challenge**: composing Gup's library takes about 0.36 s in a debug-built
  proc macro, on every expansion.
- **Solution**: one composed `Library` per thread, with `remove_module` after
  each expansion. `add_module` also rolls back on failure, so a broken function
  does not poison the next one. The two test functions that deliberately share
  `clash::scale` prove the removal.

#### naga_oil only reports the imports a module uses

- `get_preprocessor_data` drops unused `#import`s, so a first test that imported
  `gup::scale::linear` without calling it recorded no import. The test now calls
  `symlog::log2_1p`.

### Architectural Decisions

#### The attribute completes `impl ShaderFn`, not a free function

- **Decision**: `#[wgsl_function(…)] impl ShaderFn for T { fn params … }`.
- **Reasoning**: S5a settled `ShaderFn` with state-bearing hooks (`fit_domain`,
  `fit_keys`, `chunk_base`, `resources`). A free function cannot hold a domain
  or the key a `fit_keys` looks up. Writing the hooks as ordinary trait methods
  needs no new syntax.
- **Trade-off**: the default import path comes from the type name, not a
  function name as the story said. The `Params` twin's name (`<Type>Params`) is
  a convention the user learns from the docs.

#### Pad user `Params`, keep the library strict

- **Decision**: user modules are padded in the flattened text, and the twin
  hides the padding; library modules are still rejected.
- **Reasoning**: hand-written `pad_a: u32` members, filled again in Rust, were
  the clumsiest part of the first example. The library's rule protects
  hand-written encase twins, which users do not have.
- **Trade-off**: AC3 asked for rejection. The flattened WGSL differs from the
  authored WGSL by appended members (IR, not text).

#### Library sources in gup-wgsl

- **Decision**: `crates/gup-wgsl/library`, not a `gup-shaders` crate.
- **Reasoning**: both consumers already enable gup-wgsl's `compose` feature, and
  its run-time half never includes the text.
- **Future**: a library module is still "add a file", now in gup-wgsl.

### Development Workflow Insights

- **Writing the examples as a newcomer found three rough edges.** I hit WGSL
  reserved words (`smooth`, `from`), and the trailing-digit rule with `d0`/`d1`.
  A colour function's compile error also cascaded into three unrelated type
  errors. The first two produce clear messages; the third and the rename
  suggestion were fixed.
- **Disk.** Each of the seven Rust commits' hook runs consumed 2.1–2.5 GB
  (2,483, 2,113, 2,112, 2,175, 2,086, 2,192 and 2,071 MB; the first ran the full
  `mask all-check`), almost all of it debug incremental caches left by clippy
  over the root crate. Docs-only commits cost nothing. Three times free space
  dipped below 5 GB during a hook or `mask all-check` (3.1, 4.5 and 3.2 GB), and
  pruning `debug/incremental` and superseded test binaries brought it back to
  5.3–9.5 GB each time. No `cargo clean`. Pruning before every Rust commit, not
  after, is the habit to keep.
- **`perf-budget`**: a first run after rebuilding the release harness hits
  Mesa's cold shader cache (`cold_ms` 10.8). The second run is the measurement.

### Follow-up Stories

None written. Each gap maps to a planned RFC-001 step, recorded in "S5c findings
→ Proposed adjustments":

- **S6**: `derive(Mark)` can reuse `Library::add_module`, the cached library and
  the twin generation. Fill opacity and stroke must compose with user colour
  functions.
- **S7**: legends for chains (`Log.then(viridis)` has none; the aftershock
  golden places one by hand), a `legend()` for user colour functions, and a
  helper for user axis scales (`PositionScale`).
- **S14**: rewrite `docs/tutorials/03_custom_shader_functions.md`, which still
  describes the old path's macro.
