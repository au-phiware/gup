---
implementer: sonnet
implementer-reason: macro syntax, generated items and checks settled in
  Decisions; expansion follows #[wgsl_function] (load, with_library, module static, const asserts)
---

# GUP-422: RFC-001 Step S6a: `#[derive(Mark)]` v2

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-11

## Context

[RFC-001](../rfcs/RFC-001_Core_Architecture.md) §4 specifies `#[derive(Mark)]`
v2: a derive that generates a mark's `Channel<M, V>` associated constants, its
`Mark` impl, a typed attribute-setter trait, and a compile-time check (naga, at
macro expansion) that the mark's WGSL module declares an input struct matching
`CHANNELS` field for field. Today `crates/gup-core/src/marks.rs` has one mark,
`Circle` (`pub enum Circle {}`), whose `impl Mark`, four `ChannelDesc` entries
and `wgsl_input_struct_matches_channels` test are all hand-written.

[GUP-420](GUP-420_RFC_001_S5c_Wgsl_Function_Macro_V2.md) built the machinery
this derive reuses: `crates/gup-core-macros` composes authored WGSL against
Gup's library at expansion (`gup_wgsl::compose::Library`, cached per compiler
thread by `with_library`), validates it with naga, and emits a `WgslModule`
static plus `const` assertions. RFC-001's "S5c findings → Proposed adjustments
to S6" says the mark derive "can follow this pattern exactly".
[GUP-421](GUP-421_Analytic_AA_Rules_Rects_1x_MSAA.md) added `Mark::ANTIALIAS`
(`Analytic` by default, `Msaa` for marks whose edges are arbitrary triangles)
and `gup::coverage` (`crates/gup-wgsl/library/coverage.wgsl`: `rect_quad`,
`line_quad`, `box_coverage`), which this story's test mark uses.

This story was split on 2026-10-16 so each part fits one session: the derive
itself (this story), `Rect` with scale-derived widths
([GUP-429](GUP-429_RFC_001_S6c_Rect_Mark_And_Derived_Constants.md)), stroke and
fill opacity ([GUP-430](GUP-430_RFC_001_S6d_Mark_Stroke_And_Fill_Opacity.md)),
and draw order ([GUP-431](GUP-431_RFC_001_S6e_Mark_Draw_Order.md)). Circle's
channels do not change here: the derive must reproduce today's `CHANNELS`
exactly.

## Read First

- RFC-001 §4 "Typed channels" (lines 259–318), especially "What
  `#[derive(Mark)]` generates".
- RFC-001 "S5c findings" → "Proposed adjustments to S6, S7 and later" (lines
  2993–3014).
- `crates/gup-core/src/marks.rs` (all 92 lines).
- `crates/gup-core/src/channel.rs` lines 59–100 (`GpuType`, `Visual`, `Px`) and
  174–258 (`ConstValue`, `Role`, `ChannelDesc`, `Antialias`, `Mark` and its doc
  comment).
- `crates/gup-core-macros/src/lib.rs` lines 1–42 (entry point, error stub),
  228–234 (`static_name`), 250–331 (`load`, `LIBRARY`, `with_library`), 333–420
  (`expand`: import path rules and composition) and 540–640 (module static and
  `const` assertions).
- `crates/gup-wgsl/library/circle.wgsl` (the mark contract) and
  `crates/gup-wgsl/library/coverage.wgsl`.
- `crates/gup-core/src/selection.rs` lines 217–228 (`Selection::attr`).
- `crates/gup-core/tests/compile_fail.rs` (trybuild layout).

## User Story

> "As a visualization developer, I want to define a new mark by writing its WGSL
> module and a `#[derive(Mark)]`-annotated struct, with the channel constants,
> attribute setters and the WGSL-matches-channels check generated for me, so
> that adding a mark costs a shader and a few lines of Rust, not a hand-written
> trait impl and its own test."

## Acceptance Criteria

### AC1: The derive reproduces `Circle` exactly

- [ ] `Circle` in `crates/gup-core/src/marks.rs` is a `#[derive(Debug, Mark)]`
      struct (Decisions, "Circle") with no hand-written `impl Mark`.
- [ ] A new unit test asserts `Circle::CHANNELS` equals the pre-story table
      field by field (name, role, wgsl_type, default; copied into the test as
      literals), and `NAME == "Circle"`, `VERTICES_PER_INSTANCE == 6`,
      `ANTIALIAS == Antialias::Analytic`.
- [ ] The existing tests `channel_consts_index_the_descriptor_table` and
      `wgsl_input_struct_matches_channels` pass unchanged.
- [ ] Every gup-core golden passes without re-blessing:
      `cargo test -p gup-core -- --test-threads=1`.

### AC2: Generated attribute-setter trait

- [ ] `CircleAttrs<T>` is generated and implemented for `Selection<T, Circle>`:
      a doctest on `Circle` builds a plot with
      `.x(..).y(..).radius(Px(4.0)).fill(..)` (trait in scope via the prelude)
      and renders it.

### AC3: A user-defined mark works end to end

- [ ] An integration test `crates/gup-core/tests/derive_mark.rs` defines a
      `Square` mark with inline WGSL (`#[mark(wgsl = r#"…"#, geometry = Quad)]`,
      channels `x`, `y`, `size: Px`, `fill: Color`), drawn with
      `gup::coverage::rect_quad`/`box_coverage`, adds it to a `Plot` with
      explicit domains, and renders 320×200.
- [ ] For each of three squares (size 20 px, fills `#0072B2`, `#D55E00`,
      `#009E73`), the pixel at its centre equals its fill within 2/255 per
      channel, and the pixel 3 px outside its right edge is the white background
      within 2/255.

### AC4: Mismatches fail at compile time

- [ ] `crates/gup-core/tests/derive_mark_fail/` holds trybuild cases, run by a
      new `derive_mark_mistakes_do_not_compile` test in `compile_fail.rs`:
      `missing_field.rs` (a channel the WGSL `<NAME>In` lacks),
      `wrong_order.rs`, `wrong_type.rs` (a `Color` field where WGSL has `f32`),
      `unknown_module.rs` (`module = "gup::marks::nope"`),
      `selection_method_clash.rs` (a field named `len`).
- [ ] Each `.stderr` names the mark and the mismatch, e.g. "`Square`'s channel
      `fill` is `Color` (WGSL `vec4<f32>`) but `SquareIn.fill` is `f32`"; the
      unknown-module message lists the library's `gup::marks::*` modules.

### AC5: The `clip` contract documents both hiding mechanisms

- [ ] The `Mark` trait doc comment (`crates/gup-core/src/channel.rs`, lines
      236–246) describes both ways a row is hidden: a null numeric non-colour
      channel via validity bits (RFC-001 S4b), and a key-encoded non-colour
      channel equal to `NULL_CODE` (RFC-001 S5b; `glue.rs`, the `geometry_keys`
      loop near line 493). Both move every vertex to one point outside the clip
      volume. The sentence "Hand-written for now; RFC-001 S6 generates this
      impl" is replaced by a pointer to `#[derive(Mark)]`.

## Decisions

- **Where**: the derive is
  `#[proc_macro_derive(Mark, attributes(mark, channel))]` in
  `crates/gup-core-macros/src/lib.rs`, implemented in a new
  `crates/gup-core-macros/src/mark.rs`. `load`, `with_library`, `static_name`,
  `snake_case` and `is_ident` become `pub(crate)` and are shared. gup-core
  re-exports it as `pub use gup_core_macros::Mark;` next to `wgsl_function`
  (derives and traits live in different namespaces, so `gup_core::Mark` names
  both, as with serde's `Serialize`). Generated code names `::gup_core`, which
  works inside gup-core through its `extern crate self as gup_core`.
- **Syntax** (only a non-generic struct with named fields):

  ```rust
  #[derive(Debug, Mark)]
  #[mark(module = "gup::marks::circle", geometry = Quad)]
  pub struct Circle {
      /// Centre x (logical pixels).
      #[channel(role = X)]
      x: Px,
      /// Centre y (logical pixels, y down).
      #[channel(role = Y)]
      y: Px,
      /// Radius (logical pixels). Default 3.5.
      #[channel(role = Size, default = Px(3.5))]
      radius: Px,
      /// Fill colour. Default Okabe-Ito blue.
      #[channel(role = Color, default = Circle::DEFAULT_FILL)]
      fill: Color,
  }
  ```

  `#[mark(..)]` keys: exactly one of `module = "gup::…"` (a module of Gup's
  library) or `wgsl = "…"` / `file = "…"` (a user module, loaded with `load`
  exactly as `#[wgsl_function]` does); `path = "…"` only with `wgsl`/`file`
  (default `<crate>::<snake_case(Name)>`, `gup::` rejected, as in
  `#[wgsl_function]`); `geometry = Quad` (required, the only value:
  `VERTICES_PER_INSTANCE = 6`); `antialias = Msaa` (optional; omitted means the
  trait default `Analytic`). `#[channel(..)]` keys, both optional:
  `role = X | Y | Size | Color`;
  `default = <const expression of the field's type>`.

- **Generated items**:
  - `impl Name { pub const FIELD: Channel<Name, Ty> = Channel::new(i, "field"); … }`
    in field order, the const named by upper-casing the field, carrying the
    field's `#[doc]` attributes.
  - `impl Mark for Name` with `NAME = "Name"`, `MODULE`,
    `CHANNELS = &[ChannelDesc { name: "field", role, wgsl_type: <Ty as GpuType>::WGSL, default }, …]`,
    `VERTICES_PER_INSTANCE = 6` and, if given, `ANTIALIAS`.
  - `default`: `{ let v: Ty = <expr>; v.const_value() }`; with no `default`,
    `<Ty as Visual>::ZERO`. This needs two additions to `channel.rs`: an
    inherent `pub const fn const_value(self) -> ConstValue` on `Px` and `Color`,
    and an associated const `const ZERO: ConstValue;` on `Visual` (`F32(0.0)`
    for `Px`, `Vec4([0.0; 4])` for `Color`). Trait methods are not callable in
    `const` context, which is why `to_const` cannot be used.
  - `pub trait NameAttrs<T>` (the struct's visibility), one method per field:
    `fn field<E, K>(&mut self, encoding: E) -> &mut Self where E: IntoEncoding<T, Ty, K>;`,
    implemented for `Selection<T, Name>` where `T: Send + Sync + 'static` by
    forwarding to `self.attr(Name::FIELD, encoding)`. A field named like a
    `Selection` inherent method (`new`, `max_chunk_rows`, `len`, `is_empty`,
    `retain`, `rows_released`, `attr`) is a compile error at the field.
  - Inside `const _: () = { … };`, an
    `impl Name { #[allow(dead_code)] fn __gup_mark_fields(&self) { let _ = (&self.f1, …); } }`,
    so the descriptor fields do not warn as never read (measured: rustc treats
    `allow(dead_code)` items as live).
  - `MODULE`: for `module = "gup::x::y"`,
    `&::gup_core::__private::wgsl::<static_name>` (e.g. `MARKS_CIRCLE`); for a
    user module, a `WgslModule` static emitted as `#[wgsl_function]` emits its
    own (lines 545–618), with `include_str!` tracking for `file`.
- **The check**, at expansion: compose the module (library module: the cached
  library's `closure_wgsl(path)`; user module: `add_module`, read,
  `remove_module`), validate with `validate_wgsl`, find the struct whose flat
  name ends in `<NAME>In`, and require functions `vertex` and `shade`. Member
  names and order are compared in the macro (error at the struct or field span).
  Member types are compared by emitting, per field,
  `const _: () = assert!(str_eq(<Ty as GpuType>::WGSL, "<naga type>"), "<message>");`
  (the `out_check` pattern, lines 568–583), so any `Visual` type works.
- **Circle**: becomes the struct above (private fields, so it still cannot be
  constructed outside gup-core); `DEFAULT_FILL` stays an inherent const in a
  separate `impl Circle`. The hand-written impl is deleted. `marks.rs`' module
  doc drops "and the `#[derive(Mark)]` generator are RFC-001 S6".
- **trybuild snapshots** in `tests/compile_fail/` may change only where rustc
  now says "struct `Circle`" for "enum `Circle`"; regenerate them with
  `TRYBUILD=overwrite` and review the diff.
- **Prelude** (`crates/gup-core/src/lib.rs` line 251): add `CircleAttrs` (each
  built-in mark's generated trait) and the `Mark` derive, as it lists
  `wgsl_function` today.

## Escalation Triggers

- Reproducing `Circle::CHANNELS` exactly needs a change to `ChannelDesc`,
  `Mark`, `Role` or `ConstValue` beyond `const_value`/`ZERO`.
- The `Square` test composes at expansion but fails at run time in the glue
  (`shader::link` or pipeline creation): the glue may assume a library mark
  module. Report the error instead of changing `glue.rs`.
- A trybuild snapshot under `tests/compile_fail/` or `tests/wgsl_function_fail/`
  changes in any way other than the `enum` → `struct` wording.
- The derive needs a new crate dependency.

## Out of Scope

- `Rect` and scale-derived constants: GUP-429.
- Stroke, fill opacity and the `Unit` visual type: GUP-430.
- Size-descending and emphasis draw order: GUP-431.
- `Segment`: GUP-423. `Line`: GUP-432.
- Geometries other than `Quad` (areas, polygons): not yet a story.

## Technical Tasks

- [ ] Make the shared helpers in `gup-core-macros/src/lib.rs` `pub(crate)`; add
      `mark.rs` with attribute parsing, composition and checks.
- [ ] Add `Px::const_value`, `Color::const_value` and `Visual::ZERO` in
      `channel.rs`.
- [ ] Emit the consts, `Mark` impl, `…Attrs` trait and impl, field reader,
      module static (user modules) and type assertions.
- [ ] Rewrite `Circle`; add the CHANNELS-equality test; add `CircleAttrs` to the
      prelude; re-export the derive.
- [ ] Update the `Mark` trait doc (AC5) and `marks.rs`' module doc.
- [ ] Add `tests/derive_mark.rs` (AC3) and `tests/derive_mark_fail/` (AC4).

## Dependencies

### Prerequisite Stories

- GUP-420: RFC-001 S5c `#[wgsl_function]` v2 ✅ — `gup-core-macros`, library
  composition at expansion, module statics.
- GUP-421: Analytic AA for Rules and Rects ✅ — `Mark::ANTIALIAS`,
  `gup::coverage` for the test mark.
- GUP-419: RFC-001 S5b ✅ — the `NULL_CODE` compare AC5 documents.

### Enables Stories

- GUP-429 (`Rect`), GUP-423 (`Segment`), GUP-430 (stroke), GUP-432 (`Line`): all
  define marks with the derive.

## Testing Strategy

- **Unit tests**: CHANNELS equality for `Circle`; existing marks.rs tests.
- **Integration tests**: `derive_mark.rs` renders a user mark and checks pixels;
  trybuild compile-fail cases.
- **Visual validation**: none new (Circle's goldens must be unchanged).
- **Performance**: none; the derive runs at compile time.

## Success Metrics

- [ ] `marks.rs` has no hand-written `impl Mark`.
- [ ] A user mark compiles and draws from an external crate (the integration
      test) with only a struct, its WGSL and the derive.

## Risk Assessment

- **Medium**: naga's spelling of member types (`vec4<f32>` vs `vec4f`) may not
  match `GpuType::WGSL`. _Mitigation_: normalise naga's type to the spelling
  `GpuType::WGSL` uses before emitting the assertion; `convention.rs` already
  maps naga types for `Params`.
- **Low**: flat names of user-module structs differ from library ones.
  _Mitigation_: match on the `<NAME>In` suffix, as specified.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
