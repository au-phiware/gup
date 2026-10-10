# GUP-424: RFC-001 Step S7a: Multi-Layer Plot and Shared Scales

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-11

## Context

`crates/gup-core/src/plot.rs`'s module doc says its current scope is "exactly
one layer... Multiple layers with unioned domains, legends, grids, axis titles,
themes and the object-safe `Chart` trait are RFC-001 S7", and `Plot::resolve`
enforces that today:
`let [layer] = self.layers.as_mut_slice() else { return Err(...) }` fails with
any count other than one (`crates/gup-core/src/plot.rs:197-207`). `Plot` already
has a `layers: Vec<Box<dyn Layer>>` field and an `add` method that pushes onto
it (`plot.rs:118,157-164`), so the storage is multi-layer-shaped; what is
missing is `resolve` actually unioning domains across layers, drawing more than
one layer's items into the `Scene` in z-order, and — per RFC-001 §8 — the
object-safe `Chart`/`ChartExt` trait that generalises `Plot`'s current inherent
`render`/`resolve` methods so a multi-panel or facet composition (later) can
hold `Box<dyn Chart>` children.

[GUP-419](GUP-419_RFC_001_S5b_Colour_Scales_And_Dictionary_Hook.md)'s
retrospective found the specific bug that makes "multi-layer" more than a
resolve-loop change: "each key channel's encoding (`KeyEncoded`) now owns its
`Dictionary<K>`" (S5b, moving dictionaries out of the column store because a
generic key type cannot live in a type-erased store). That is correct for one
layer, but RFC-001's "S5b findings → Proposed adjustments to S5c, S6, S7"
section states the consequence directly: "two layers sharing one
`ScaleRef<Categorical>` give the same key different codes... Multi-layer plots
need one dictionary per shared scale, the union of their layers' keys...
`KeyEncoded::domain` is the single-layer workaround." Concretely: today,
`ScaleRef<S>::fit_keys` (`crates/gup-core/src/scale/mod.rs:140-142`) just
forwards to `self.write().fit_keys(keys)`, and each layer's `KeyEncoded` calls
it with its own `Dictionary`'s keys independently
(`crates/gup-core/src/encoding.rs:347`). If layer A's dictionary assigns
`"Asia" → 0` and layer B's dictionary (built separately, possibly in a different
row order) assigns `"Asia" → 1`, then `Categorical`'s shared palette LUT draws
`"Asia"` in two different colours depending on which layer's row is being shaded
— the shared `ScaleRef` was supposed to make that impossible. The orchestrator's
review of GUP-419 called this "the gap that most needs an owner": without a fix,
any composite chart with two layers colour-coded by the same categorical field
(the RFC-001 §10(d) composite example, extended with a `color` encoding)
silently miscolours one of them.

This story is deliberately scoped to the structural layer: multiple layers
drawing with correct shared domains and correct shared dictionaries, plus the
`Chart`/`ChartExt` trait that RFC-001 §8 specifies as the uniform entry point
every builder and every render target will use from S10 onward. Guide quality
(axis titles, grid, `Theme`, tick density, log-domain niceness) is
[GUP-425](GUP-425_RFC_001_S7b_Axes_Theme_And_Tick_Quality.md) (S7b); legend
placement is [GUP-426](GUP-426_RFC_001_S7c_Legend_Placement.md) (S7c). Both
depend on this story because they need more than one layer's worth of ticks,
colours and keys to have something to place.

## User Story

> "As a visualization developer, I want to add more than one data layer to a
> `Plot`, with their domains unioned and any scale they share — including a
> categorical colour scale — giving every layer's rows the same code for the
> same key, so that a composite chart (for example a forecast line plus observed
> points) renders correctly instead of erroring or miscolouring."

## Acceptance Criteria

### AC1: `Plot::resolve` accepts more than one layer

- [ ] `Plot::resolve` no longer errors for 2+ layers; it fits domains across
      every layer sharing a scale (not just the first), takes the union of their
      f64 chunk stats for `x`/`y`, then `nice()`s as today.
- [ ] Each layer's `ItemKind::Marks` scene item is emitted with the z-order
      RFC-001 §7 specifies (insertion order by default, with each layer's own
      `.z(i32)` override already supported or added here if missing) — a test
      with layers in reverse-visual-order but explicit `.z()` values confirms
      the later-`z` layer draws on top.
- [ ] A golden (`gup_core/composite_two_layers` or similar) renders a line layer
      and a scatter layer sharing one x/y domain (for example RFC-001 §10(d)'s
      forecast line plus observed points) and, read by eye, both layers are
      visible, correctly positioned relative to each other, and the scatter
      layer draws on top per its `.z()`.

### AC2: Shared categorical/key scales give one code per key across layers

- [ ] A `ScaleRef<Categorical>` (or any key-domained scale) shared by two
      layers' `color` (or other key-encoded) channels resolves to **one**
      dictionary, built from the union of both layers' keys, so the same key
      string gets the same code — and therefore the same palette colour — in
      both layers' drawn rows.
- [ ] A test with two layers, each containing a key the other does not have plus
      one key both share, confirms: the shared key draws in the same palette
      colour in both layers; each layer's own unique key still gets a colour
      (not an error, not a fallback to null); the legend (if present) lists the
      union, not a duplicate or a missing entry.
- [ ] `KeyEncoded::domain(keys)` continues to work as the documented
      single-layer (or pre-seeded multi-layer) workaround; this AC does not
      remove it, it makes the undeclared multi-layer case correct too.

### AC3: The `Chart` / `ChartExt` trait

- [ ] An object-safe `Chart` trait exists (RFC-001 §8):
      `resolve(&mut self, cx:     &mut ResolveCx<'_>) -> Result<&Resolved>`
      (cached; cheap when the inputs are unchanged), plus `as_any`/`as_any_mut`
      for the existing downcast-based layer access pattern. `Plot` implements
      it.
- [ ] A `ChartExt` blanket impl (for `T: Chart + ?Sized`, so it also works for
      `dyn Chart`) provides `save_png`, `to_rgba` (or the sync equivalent
      already in use), and `layout()`, generalising `Plot`'s current inherent
      `render` method without removing it (or replacing it with a thin
      forwarding call — document which).
- [ ] A test exercises `Chart`/`ChartExt` through a `Box<dyn Chart>` (not only
      through the concrete `Plot` type), proving the trait is actually
      object-safe and usable where RFC-001 §8's "multi-panel and facets" note
      requires it to be.

### AC4: Resolve caching

- [ ] A second `resolve` call with no change to data, domains or size returns
      the cached `Resolved` (RFC-001 §9's resolve pipeline: "runs only when the
      size, data or a domain changes") rather than re-running the full pipeline;
      a test proves this by a counter or a deliberately expensive step that must
      not re-run, and proves a genuine change (new row appended, size changed)
      does invalidate the cache.

## Technical Tasks

- [ ] Remove the single-layer guard in `Plot::resolve`; implement the
      domain-union loop across all layers sharing a scale.
- [ ] Move or share `Dictionary<K>` state so a `ScaleRef<S>` shared by two
      `KeyEncoded` channels resolves to one dictionary — likely by having
      `ScaleRef` (or a new `SharedDictionary<K>` it owns per key-domained scale)
      hold the canonical dictionary, with each `KeyEncoded` writing through it
      rather than owning a private copy; audit `fit_keys`'s call sites
      (`crates/gup-core/src/scale/mod.rs:140`,
      `crates/gup-core/src/encoding.rs:347`) and `KeyEncoded` (`encoding.rs:496`
      onward) to find the smallest change that preserves S5b's "a generic key
      cannot live in a type-erased store" constraint while fixing the
      two-dictionaries bug.
- [ ] Define `Chart`/`ChartExt`/`ResolveCx`/`Resolved` per RFC-001 §8; migrate
      `Plot`'s inherent methods to implement/call through the trait.
- [ ] Add per-layer `.z(i32)` if not already present; confirm insertion-order
      tiebreak.
- [ ] Add the resolve-cache invalidation logic (dirty flags on data, domain,
      size) if not already implicit in the current single-layer path.
- [ ] Add goldens: `composite_two_layers`, a shared-categorical-colour test
      scene.
- [ ] Update `plot.rs`'s module doc comment to drop the "exactly one layer"
      scope note.

## Dependencies

### Prerequisite Stories

- GUP-419: RFC-001 S5b Colour Scales and Dictionary Hook ✅ — the dictionary
  hook and per-channel `Dictionary<K>` design this story's shared-dictionary fix
  builds on; also the source of the "one dictionary per shared scale" finding
  this story resolves.
- GUP-414: RFC-001 S4a Column Store Chunking and Append ✅ — multi-chunk column
  stats this story's domain-union loop reads per layer.
- GUP-401: RFC-001 S3 Scene, Renderer, RenderTarget ✅ — `Scene`, `ItemKind`,
  z-order constants (`Z_GRID`, `Z_AXES`, `Z_TITLE`) this story's multi-layer
  emission follows.

### Soft Dependencies

- GUP-422: RFC-001 S6a `#[derive(Mark)]` v2, Circle and Rect 📋 — not required
  (this story's goldens can use `Circle` alone), but a second mark makes
  `composite_two_layers` a more realistic test of multi-layer rendering (for
  example a `Rect`-based bar layer plus a `Circle` scatter layer) if it has
  already landed.

### Enables Stories

- GUP-425: RFC-001 S7b Axes, Theme and Tick Quality — needs more than one
  layer's domains and keys to size axes, grid and tick density correctly.
- GUP-426: RFC-001 S7c Legend Placement — needs the shared-dictionary fix so a
  legend built from a shared categorical scale lists the union of keys with one
  colour per key, not a per-layer legend that could disagree with itself.
- RFC-001 S11 (composite/mixed-`T` layers; not yet written as a story) — builds
  on this story's multi-layer resolve and the `Chart` trait.

## Testing Strategy

- **Unit tests**: domain-union arithmetic across layers; shared-dictionary code
  assignment (same key, same code, from two independently-ordered layers);
  resolve-cache hit/miss.
- **Integration tests**: `composite_two_layers` rendering through `Chart`'s
  object-safe interface (`Box<dyn Chart>`); the two-layer shared-categorical
  test from AC2.
- **Visual validation**: `composite_two_layers` golden read by eye — both layers
  visible, correctly positioned, correct z-order.
- **Performance**: resolve-cache test proves no redundant work on an unchanged
  resolve; record `mask perf-budget` impact if the domain-union loop measurably
  changes resolve cost at realistic layer counts (2–5).

## Success Metrics

- [ ] A composite chart with 2+ layers resolves and renders correctly, read by
      eye.
- [ ] A shared categorical scale across two layers gives one key one colour,
      proven by a seeded test (two layers, overlapping and non-overlapping
      keys).
- [ ] `Chart`/`ChartExt` is usable through `Box<dyn Chart>` in a test.

## Risk Assessment

- **High**: the shared-dictionary fix touches the exact design S5b just settled
  (each `KeyEncoded` owning its own `Dictionary<K>`, chosen because "a generic
  key cannot live in a type-erased store"). Moving dictionary ownership to the
  shared `ScaleRef` without breaking that constraint, and without breaking S5b's
  single-layer `encode_owned_key`/`domain` API, is the hardest part of this
  story. _Mitigation_: `ScaleRef<S>` is already generic over `S`, not
  type-erased at the point `fit_keys` is called, so a `ScaleRef`-owned
  dictionary (keyed by the scale's own type, not a row type) may thread the
  needle; prototype against AC2's test before committing to an API shape, and be
  willing to revise `KeyEncoded`'s internals (not necessarily its public
  surface) more than the AC implies if the first design does not work.
- **Medium**: `Chart`/`ChartExt` as an object-safe trait with a cached `resolve`
  returning `&Resolved` has lifetime implications for callers holding the
  reference across a mutable borrow. _Mitigation_: follow RFC-001 §8's sketch
  closely; it was written with object-safety and the cache in mind.
- **Low**: per-layer `.z()` may already exist in some form on `Selection`; check
  before adding a duplicate mechanism.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
