# GUP-422: RFC-001 Step S6a: `#[derive(Mark)]` v2, Circle and Rect

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-11

## Context

[RFC-001](../rfcs/RFC-001_Core_Architecture.md) §4 specifies `#[derive(Mark)]`
v2: a proc macro that generates a mark's `Channel<M, V>` associated constants,
its `Mark` impl (`NAME`, `MODULE`, `CHANNELS`, `VERTICES_PER_INSTANCE`), a typed
attribute-setter trait, and a compile-time check (naga, at macro expansion) that
the mark's WGSL module declares an input struct matching `CHANNELS` field for
field. Today `crates/gup-core/src/marks.rs` has exactly one mark, `Circle`,
written by hand: its `Mark` impl, its four `ChannelDesc` entries and its
`wgsl_input_struct_matches_channels` test all exist only because S0a wrote them
directly (the module's own doc comment: "Rect, Rule and Segment/Line and the
`#[derive(Mark)]` generator are RFC-001 S6"). The channel type machinery this
macro targets (`Channel`, `GpuType`, `Visual`, `ConstValue`,
`channel.rs:216-232`'s `Mark` trait contract) is already in place and used by
`Circle`; this story is the macro that stops a second mark from repeating
`Circle`'s ~90 lines of boilerplate and its hand-maintained struct-matches-
channels test by hand.

[GUP-419](GUP-419_RFC_001_S5b_Colour_Scales_And_Dictionary_Hook.md)'s
retrospective recorded three gaps in `Circle` itself that this story also
closes, because they are properties of the mark contract and geometry, not of
any one mark: "fill opacity and stroke (for S5a's opaque `sqrt_radius` discs and
the near-white diverging midpoint), size-descending z-order, and a bar width
that follows `Band::band_width()`." Today `circle.wgsl`'s `shade()`
(`crates/gup-core/src/shaders/circle.wgsl:58-67`) already premultiplies by
`fill.a`, so opacity is technically available, but no example or golden uses it
and there is no stroke channel at all: a `sqrt_radius` bubble chart draws solid
discs, so a large disc fully occludes a smaller one drawn under it, and
[RFC-001's "S5b findings"](../rfcs/RFC-001_Core_Architecture.md) records that
`diverging_anomaly`'s near-white midpoint colour (`#f7f7f7`) "almost disappears
on a white plot" and had to be worked around with a grey plot background rather
than fixed. Both are solved by the same feature: every mark gets `STROKE` and
`STROKE_WIDTH` channels, drawn as a ring at the shape's edge. Draw order is a
separate problem: `gup-core` draws a layer's rows in upload order, so without a
size-descending order a late-inserted small bubble can still end up on top of an
earlier large one only by chance of insertion order, not by design — a bubble
chart wants the largest marks drawn first so smaller ones are never hidden. A
`Rect` mark needs a width that is not a per-row column at all for the common
case: a bar's width should be `Scale::band_width()`, one value from the scale
that changes when the domain changes (a new category appended), not a constant
baked in at `attr` time.

[GUP-421](GUP-421_Analytic_AA_Rules_Rects_1x_MSAA.md) (📋 Planned, not yet
implemented) gives `rect.wgsl`'s fragment shader analytic edge coverage for the
guide-only `ItemKind::Rects` path. This story's `Rect` mark is a new, separate
WGSL module (`gup::marks::rect`, per-row columns and the mark contract) rather
than a reuse of that guide shader, but it should follow the same edge-coverage
technique for its stroke ring once GUP-421 lands, rather than inventing a third
approach; see the soft dependency below.
[RFC-001's "S5b findings" → "Proposed adjustments to S5c, S6, S7"](../rfcs/RFC-001_Core_Architecture.md)
also records that "the `clip` contract now also carries the `NULL_CODE`
compare": a position or size channel bound to a key (for example a `Band`-scaled
x) hides a null row by the code comparison the glue already emits
(`crates/gup-core/src/shader/glue.rs:493`), not only by the off-screen-point
trick S4b used for numeric nulls. `channel.rs`'s `Mark` trait doc should say so
directly, and the macro's compile-time check should know both hiding mechanisms
exist.

## User Story

> "As a visualization developer, I want to define a new mark by writing its WGSL
> module and a `#[derive(Mark)]`-annotated struct, with the channel constants,
> attribute setters and the WGSL-matches-channels check generated for me, so
> that adding a mark costs a shader and a few lines of Rust, not a hand-written
> trait impl and its own test."

A second role also benefits:

> "As a visualization developer using `Circle` or `Rect` for a bubble chart or
> scatter plot, I want semi-transparent fills, an optional stroke, and the
> largest marks drawn first, so that overlapping points stay readable and a
> near-white fill is still visible against a white background."

## Acceptance Criteria

### AC1: `#[derive(Mark)]` generates the boilerplate `Circle` wrote by hand

- [ ] `#[derive(Mark)] #[mark(module = "gup::marks::rect", geometry = Quad)]     struct Rect { #[channel(role = X)] x: Px, ... }`-style
      annotations produce: `Channel<Rect, V>` associated constants in
      declaration order (slot assigned by position), a `Mark` impl (`NAME`,
      `MODULE`, `CHANNELS` built from the annotations' roles/types/defaults,
      `VERTICES_PER_INSTANCE`), and a typed attribute-setter trait
      (`RectAttrs<T>`, one method per channel) mirroring `CircleAttrs<T>`'s
      shape if one already exists, or establishing it if not.
- [ ] `Circle` (`crates/gup-core/src/marks.rs`) is rewritten to use the derive
      macro instead of its hand-written `impl Mark for Circle`; its existing
      tests (`channel_consts_index_the_descriptor_table`,
      `wgsl_input_struct_matches_channels`) keep passing, proving the macro
      reproduces today's hand-written behaviour exactly.
- [ ] A naga-based compile-time check (at macro expansion, like the existing
      `wgsl_input_struct_matches_channels` test but running for every mark, not
      just as a test) verifies the mark's WGSL module declares an input struct
      (`<NAME>In`) whose fields match `CHANNELS` in name, order and WGSL type. A
      mismatch (missing field, wrong type, wrong order) fails `cargo build` with
      a message naming the mark and the mismatch, not a runtime panic or a
      silent wrong draw.
- [ ] A `trybuild` test proves the mismatch case fails at compile time with a
      readable message.

### AC2: `Rect` mark, with a scale-derived width

- [ ] A new `Rect` mark (`gup::marks::rect` WGSL module, distinct from the
      guide-only `rect.wgsl`/`RECT_SHADER` path) draws an axis-aligned rectangle
      per row from `X`, `Y` (one corner or the centre — document which) and
      `WIDTH`/`HEIGHT` channels, plus `FILL`, `STROKE`, `STROKE_WIDTH` (AC3).
- [ ] `WIDTH` (or `HEIGHT`, whichever axis a `Band` scale occupies) accepts a
      scale-derived constant: binding `Rect::WIDTH` to `x_scale.band_width()`
      (or an equivalent `attr` call reading the scale rather than a per-row
      accessor) produces one uniform value that is re-derived whenever the
      scale's domain changes (a new category appended), without a per-row column
      and without the caller hard-coding a pixel width.
- [ ] A test appends a new category to a `Band`-scaled x and asserts the next
      `resolve`'s rect width uniform shrinks to fit the wider domain, with no
      new column bytes written for `WIDTH`.

### AC3: Fill opacity and stroke on every mark

- [ ] `Circle` and `Rect` both gain `STROKE: Channel<M, Color>` (default
      `Color::TRANSPARENT`) and `STROKE_WIDTH: Channel<M, Px>` (default
      `Px(0.0)`) channels. Both shaders draw a stroke ring/border at the shape's
      edge, inside or straddling the fill edge (document which), analytically
      antialiased the same way the fill edge already is.
- [ ] A golden (`gup_core/bubble_stroke` or similar) shows several overlapping
      `Circle`s with semi-transparent fill (`fill.a < 1.0`) and an opaque dark
      stroke: read by eye, overlapping bubbles are individually distinguishable
      by their stroke outlines, and a bubble's fill colour is visibly blended
      with what is behind it.
- [ ] `diverging_anomaly` (or a new golden using `Diverging`) is re-rendered
      with a stroke on its marks instead of (or in addition to) the grey plot
      background GUP-419 used as a workaround; the near-white midpoint colour is
      now visible against a white plot background by its stroke outline, read by
      eye. The grey-background workaround may be removed if the stroke alone
      makes the midpoint legible, or kept alongside it — the story records
      which, and why, after looking at both.

### AC4: Size-descending draw order

- [ ] Rows bound to a `Size`-role channel (for example `Circle::RADIUS`) draw
      largest-first (so smaller marks are never fully occluded by a larger one
      inserted earlier), implemented as a documented mechanism (for example a
      per-row pseudo-depth derived from the size channel plus depth testing, or
      an explicit sort) — the story picks one approach and documents why,
      consistent with append-friendly, chunked upload (RFC-001 §3): the
      mechanism must not require re-sorting the column store on every append.
- [ ] A test renders rows in an adversarial insertion order (smallest row
      inserted last) at varying radii and confirms, by reading pixel colour at
      each row's known centre, that every row's centre pixel is that row's own
      fill colour, not a larger row's — i.e. nothing is hidden by insertion
      order alone.
- [ ] A `sqrt_radius`-style golden is re-rendered and read by eye: larger
      bubbles no longer visually bury smaller ones that should be on top.
- [ ] Emphasis draws on top too: when a categorical highlight (for example
      GUP-420's `wgsl_function_highlight`, where the vermilion Asian countries
      must not sit under grey ones) or an explicit `z` channel is bound, the
      emphasised rows draw last. Document how this combines with size-descending
      order (orchestrator review of the GUP-420 goldens, 2026-10-11).

### AC5: The `clip` contract documents both hiding mechanisms

- [ ] `channel.rs`'s `Mark` trait doc comment (`crates/gup-core/src/channel.rs`,
      the `Varyings`/`clip` paragraph) is updated to describe both ways a row is
      hidden: the degenerate-quad trick for a null numeric position/size
      (RFC-001 S4b) and the `NULL_CODE` equality compare the glue ANDs into
      `drawn` for a key-encoded channel (RFC-001 S5b,
      `crates/gup-core/src/shader/glue.rs:493`). The derive macro's generated
      glue (or the hand-written glue it calls into) is unaffected in behaviour;
      this AC is about the contract being documented where a mark author reads
      it, not a new mechanism.

## Technical Tasks

- [ ] Design the `#[mark(...)]` / `#[channel(...)]` attribute syntax (follow
      RFC-001 §4's sketch) and implement the derive macro in a proc-macro crate
      — reuse `crates/gup-core-macros` if GUP-420 has already created it,
      otherwise create one now and let GUP-420 add to it (coordinate; this is a
      soft dependency, see below).
- [ ] Implement the naga-based `CHANNELS`-matches-WGSL-struct check at macro
      expansion, generalising the existing `wgsl_input_struct_matches_channels`
      test's logic (`crates/gup-core/src/marks.rs`) so it runs for every derived
      mark, not just as a hand-written test for `Circle`.
- [ ] Rewrite `Circle` to use the derive macro; delete its hand-written
      `impl     Mark for Circle` once the generated one is proven equivalent.
- [ ] Write `gup::marks::rect`'s WGSL module (quad-expanded like `circle.wgsl`,
      working in logical pixels) with
      `RectIn { x, y, width, height, fill,     stroke, stroke_width }` and
      fragment-shader coverage for the stroke ring.
- [ ] Add a scale-derived constant path for `attr`: a way to bind a channel to
      `ScaleRef<Band>::band_width()` (or a small `BandWidth` wrapper type
      implementing whatever `IntoEncoding` marker is appropriate) that writes a
      uniform and is marked dirty when the scale's domain changes.
- [ ] Add `STROKE`/`STROKE_WIDTH` to `Circle`'s channel list and `circle.wgsl`'s
      fragment shader.
- [ ] Implement the size-descending draw-order mechanism (depth-based or sorted,
      per the chosen design) and wire it into the render path for `Size`-role
      channels.
- [ ] Update `channel.rs`'s `Mark` trait doc comment per AC5.
- [ ] Add/re-bless goldens: `bubble_stroke` (new), `diverging_anomaly`
      (re-rendered with a stroke), `sqrt_radius` (re-rendered with draw-order
      fixed).
- [ ] Add `trybuild` cases for the macro's compile-time channel/WGSL mismatch
      check.

## Dependencies

### Prerequisite Stories

- GUP-419: RFC-001 S5b Colour Scales and Dictionary Hook ✅ — the dictionary
  hook, `Categorical`, `Diverging`, `Band`/`Point`, and the `NULL_CODE` compare
  in the glue that AC5 documents; also the source of the opacity/stroke/
  z-order/band-width findings this story closes.
- GUP-401: RFC-001 S3 Scene, Renderer, RenderTarget ✅ — `Scene`, `ItemKind`,
  the render path this story's new `Rect` mark and stroke rendering draw
  through.

### Soft Dependencies

- GUP-420: RFC-001 S5c `#[wgsl_function]` Macro v2 📋 — not required to start,
  but if it lands first, its new proc-macro crate and its "library WGSL sources
  reachable at macro expansion" infrastructure are the natural home for this
  story's derive macro too, avoiding two proc-macro crates and two copies of the
  library-source-loading logic. If this story lands first, GUP-420 should build
  on its crate instead.
- GUP-421: Analytic AA for Rules and Rects; 1× MSAA 📋 — not required: `Rect`'s
  stroke ring can ship with MSAA-only edge quality initially. Once GUP-421
  lands, `Rect`'s stroke should be revisited to use the same analytic
  edge-coverage technique rather than diverge from it.

### Enables Stories

- GUP-423: RFC-001 S6b Line, Segment and Rule Marks — reuses the derive macro
  this story builds, rather than writing a third mark by hand.
- RFC-001 S7 stories (multi-layer `Plot`, guides, legends; GUP-424–426) —
  benefit from `Rect` existing for bar-chart-shaped goldens, and from the derive
  macro for any guide-adjacent mark they introduce.
- RFC-001 S10 (bar builder; not yet written as a story) — needs `Rect` with a
  `Band::band_width()`-derived width and `KeyEncoded::domain` for sorted
  categories (RFC-001 "S5b findings").

## Testing Strategy

- **Unit tests**: macro-generated `Channel` constants index `CHANNELS` correctly
  (as `Circle`'s existing test does); the scale-derived-width uniform recomputes
  on domain change with no column write.
- **Integration tests**: `trybuild` for the compile-time channel/WGSL mismatch;
  a render test proving size-descending draw order by reading per-row centre
  pixels against an adversarial insertion order.
- **Visual validation**: `bubble_stroke`, `diverging_anomaly`, `sqrt_radius`
  goldens read by eye; saved PNGs described in the story's retrospective.
- **Performance**: `mask perf-budget` should not regress materially — the stroke
  adds one more coverage computation per fragment and the draw-order mechanism
  should cost at most a per-row scalar, not a CPU sort on every frame; record
  the before/after numbers if they move outside noise.

## Success Metrics

- [ ] A second mark (`Rect`) exists with less hand-written boilerplate than
      `Circle` needed, proving the macro earns its keep.
- [ ] No golden regresses when read by eye; `bubble_stroke`, `diverging_anomaly`
      and `sqrt_radius` are confirmed by eye to show the intended fix.
- [ ] `mask perf-budget` passes with numbers recorded for the new stroke and
      draw-order costs.

## Risk Assessment

- **Medium**: the size-descending draw-order mechanism has more than one
  plausible design (depth buffer vs CPU sort vs GPU sort), each with different
  cost and append-friendliness trade-offs. _Mitigation_: prefer a per-row
  pseudo-depth written from the size channel at vertex-shader time, with depth
  testing enabled for mark passes — this needs no re-sort of the column store
  and costs one extra varying, but changes the render pass's depth-buffer usage,
  which should be checked against existing MSAA/pick-pass assumptions.
- **Medium**: naga-based compile-time checking inside a proc macro is new for
  this crate (GUP-420 is attempting the analogous thing for `#[wgsl_function]`
  concurrently); the two stories' proc-macro infrastructure may need
  reconciling. _Mitigation_: the soft-dependency note above; whichever story
  lands first should leave the crate boundary easy for the other to use.
- **Low**: a stroke ring's analytic coverage where it overlaps the fill edge
  (anti-aliasing two ramps at once) is more fiddly than a single edge.
  _Mitigation_: GUP-421's rect/rule edge-coverage work, once it lands, is the
  reference technique; until then, accept a slightly less precise ramp at the
  fill/stroke boundary and revisit.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
