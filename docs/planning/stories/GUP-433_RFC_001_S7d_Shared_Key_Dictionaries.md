---
implementer: sonnet
implementer-reason:
  dictionary ownership (a slot on ScaleRef), the ShaderFn hook, lock order and
  the mismatch error are settled in Decisions
---

# GUP-433: RFC-001 Step S7d: One Key Dictionary per Shared Scale

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-16

## Context

RFC-001 S5b moved dictionaries out of the column store, because a generic key
type cannot live in a type-erased store: each key channel's `KeyEncoded` owns
its `Dictionary<K>` (`crates/gup-core/src/encoding.rs` lines 494–505). RFC-001's
"S5b findings → Proposed adjustments" states the consequence: "two layers
encoding through one `ScaleRef<Categorical>` give the same key different codes,
and the scale keeps whichever keys it was fitted to last. Multi-layer plots need
one dictionary per shared scale, the union of their layers' keys, for example
held by the `ScaleRef` and handed to each channel. `KeyEncoded::domain` is the
single-layer workaround." Without it, a composite chart coloured by one
categorical field silently miscolours one layer, and two layers on one `Band`
axis put the same key in two places. GUP-419's review called it "the gap that
most needs an owner".

This was split out of
[GUP-424](GUP-424_RFC_001_S7a_Multi_Layer_Plot_And_Shared_Scales.md) on
2026-10-16. GUP-424 gives `ScaleRef` a private `Shared<S>` inner struct; this
story adds the dictionary there. `ScaleRef<S>` is not type-erased where
`encode_key` builds a `KeyEncoded`, but the key type `K::Key` is chosen by the
accessor, so the slot stores the dictionary as `dyn Any` and downcasts.
Dictionary codes are append-only (a key keeps its code), so one layer adding
keys never invalidates another layer's uploaded codes.

## Read First

- RFC-001 "S5b findings" → "The dictionary hook" (lines 2660–2693) and "Proposed
  adjustments to S5c, S6, S7" (lines 2791–2826).
- `crates/gup-core/src/encoding.rs` lines 75–104 (`fit_domain`, `fit_keys`),
  160–260 (`encode_key` and siblings), 494–560 (`KeyEncoded`, `domain`), 560–655
  (`KeySource`, `KeyAccessor` impls) and 780–810 (`KeyEncoded`'s `evaluate` and
  `fit`).
- `crates/gup-core/src/scale/mod.rs` `ScaleRef` (after GUP-424) and
  `crates/gup-core/src/column.rs` lines 150–200 (`Dictionary`).
- `crates/gup-core/tests/ordinal_png.rs` (the `categorical_species` test, near
  line 129) for a categorical colour plus legend setup.

## User Story

> "As a visualization developer, I want two layers that share a categorical or
> band scale to give each key one code, so that a key has the same colour and
> position in every layer."

## Acceptance Criteria

### AC1: Shared colour codes

- [ ] A test with two `Circle` layers sharing `ScaleRef<Categorical>` (and `x`,
      `y`): layer A has keys Asia, Europe; layer B has Africa, Asia (B's
      first-seen order differs). After `resolve` and render, every Asia row's
      centre pixel in both layers is the same colour within 2/255; Africa,
      Europe and Asia are three distinct colours.
- [ ] `colour.read().legend().swatches()` labels are `[Asia, Europe, Africa]`
      (union, in first-seen order across layers in insertion order).

### AC2: Shared band positions

- [ ] Two layers on one `ScaleRef<Band>` x with overlapping keys: the same key
      draws at the same x pixel (within 0.5 px) in both layers, and the axis has
      one tick per distinct key.

### AC3: Domains and appends

- [ ] `.domain([..])` on one layer's `encode_key` pre-seeds the shared
      dictionary: a test with layer A `.domain(["c", "b", "a"])` and layer B
      keys `a`, `d` gives `Band::keys() == [c, b, a, d]`.
- [ ] Appending rows with a new key to layer B uploads only B's new rows' bytes;
      layer A's columns are not rewritten (`UploadStats`).

### AC4: Mismatched key types fail clearly

- [ ] Layer A with `encode_key` (`Arc<str>` keys) and layer B with
      `encode_owned_key::<u32>` on one shared scale make `resolve` return
      `Error::config("shared key scale", …)` whose message names both key types
      (`std::any::type_name`).

### AC5: Single-layer behaviour unchanged

- [ ] Every existing gup-core golden and test passes without changes
      (`categorical_species`, `band_months`, `categorical_nulls` included).

## Decisions

- **Ownership**: a shared scale owns one dictionary. `scale/mod.rs`'s
  `Shared<S>` (GUP-424) gains `keys: DictionarySlot`.
- **`DictionarySlot`** (`crates/gup-core/src/encoding.rs`, `#[doc(hidden)] pub`,
  re-exported only from `gup_core::__private`):
  `pub struct DictionarySlot(Mutex<Option<(&'static str, Box<dyn Any + Send + Sync>)>>)`,
  holding a type name and an `Arc<Mutex<Dictionary<K>>>`. Method
  `pub(crate) fn get_or_insert<K: DictionaryKey>(&self) -> Result<Arc<Mutex<Dictionary<K>>>, (&'static str, &'static str)>`
  returns the existing dictionary, inserts a new one if empty, or reports
  `(existing type, requested type)` on a type mismatch.
- **Hook**: `ShaderFn` gains
  `#[doc(hidden)] fn dictionary_slot(&self) -> Option<&DictionarySlot> { None }`,
  forwarded by `DynShaderFn`; `ScaleRef` returns `Some(&shared.keys)`.
- **`KeyEncoded`**: `dictionary: Arc<Mutex<Dictionary<K::Key>>>` plus
  `mismatch: Option<String>`. `KeyEncoded::new(key, func)` takes the first
  link's slot (`func.links()[0].dictionary_slot()`): with a slot,
  `get_or_insert`; without (a scale used by value), a private dictionary. On a
  mismatch it uses a private dictionary and records the message, which its
  `evaluate` returns as the error. `Clone` shares the dictionary (it used to
  copy it; say so in the doc).
- **Lock order**: never hold a dictionary lock while calling into the scale.
  `fit` clones the labels (`Vec<Arc<str>>`), releases the lock, then calls
  `fit_dictionary`.
- **Fitting**: `fit_keys` stays "adopt these keys"; each layer passes the shared
  dictionary's full labels, so the last fit in a pass has the union. No union
  logic is needed in `fit_keys`.

## Escalation Triggers

- A `KeyEncoded` is built before its scale is a `ScaleRef` in some API path (for
  example a builder that wraps the scale later), so the slot is missed.
- Sharing changes any existing golden.
- `Dictionary` needs to become `Send + Sync` beyond what `DictionaryKey` already
  requires.

## Out of Scope

- Multi-layer resolve itself: GUP-424.
- Legend placement: GUP-426.
- Removing `KeyEncoded::domain` (it stays as the way to fix order).

## Technical Tasks

- [ ] Add `DictionarySlot`, the `ShaderFn`/`DynShaderFn` hook and the `ScaleRef`
      override.
- [ ] Change `KeyEncoded`'s dictionary to the shared form; update `domain`,
      `evaluate`, `fit`.
- [ ] Tests for AC1–AC4.

## Dependencies

### Prerequisite Stories

- GUP-424: Multi-Layer Plot and Shared Scales 📋 — several layers per plot and
  `ScaleRef`'s `Shared<S>`.
- GUP-419: RFC-001 S5b ✅ — `Dictionary<K>`, `encode_key`, `domain`.

### Enables Stories

- GUP-426: legends for a categorical scale shared by layers list each key once.

## Testing Strategy

- **Unit tests**: `DictionarySlot::get_or_insert` (empty, same type, other
  type).
- **Integration tests**: AC1–AC4 two-layer renders and errors.
- **Visual validation**: none new; AC1 checks pixels.
- **Performance**: one uncontended lock per channel evaluation; none measurable.

## Success Metrics

- [ ] No two layers sharing a key scale can give one key two codes.

## Risk Assessment

- **Medium**: this revises S5b's settled ownership. _Mitigation_: the public API
  (`encode_key`, `encode_owned_key`, `domain`) is unchanged, and AC5 guards
  single-layer behaviour.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
