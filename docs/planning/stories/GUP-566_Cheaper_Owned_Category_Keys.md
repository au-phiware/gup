---
implementer: sonnet
implementer-reason:
  a committed benchmark, then two fixed changes kept or dropped by a stated
  measurement rule; no design left open
touches: [crates/gup-core/src/encoding.rs, crates/gup-core/src/builder/]
---

# GUP-566: Cheaper Owned Category Keys

## Story Overview

**Initiative**: Strategic Review 2026-11 (N5 Performance at Scale) **Status**:
🚧 In Progress **Created**: 2026-10-26

## Context

GUP-537 made the builders take owned category keys
(`.color(|r| r.key.clone())`). Measured on 1,000,000 rows with 8 `String` keys,
the first resolve went from 63.3 ms with a borrowed key to 115.6 ms with an
owned one: about 52 ns more per keyed row (GUP-537 Implementation Summary, AC7).
A per-closure label cache did not help (119.0 ms) and was reverted.

Per keyed row the builder path does: the user's clone (a `String` allocation,
inherent to owned keys); `builder::label` copying it into a new `Arc<str>` (a
second allocation, `builder/mod.rs` lines 162–181); a SipHash lookup of that
`Arc<str>` in the channel's `Dictionary<Arc<str>>` (`column.rs` lines 184–280,
reached through `OwnedKey`'s `KeyAccessor` impl, `encoding.rs` around line 950);
then two frees. The borrowed path (`Key`, `encode_borrowed`) does only the
lookup. So the avoidable cost is the second allocation and its free; the label
cache failed because it still hashed every row and kept the clone. A faster
hasher also cuts the lookup that both paths pay.

This story commits a benchmark, makes the two changes in Decisions, keeps each
only if it measurably helps, and states a target.

## Read First

- `docs/planning/stories/GUP-537_Owned_Category_Keys_In_Builders.md`, AC7 (lines
  224–231) and the AC7 results (lines 478–490).
- `crates/gup-core/src/builder/mod.rs` lines 155–181 (`Key`, `erase_key`,
  `label`).
- `crates/gup-core/src/column.rs` lines 145–280 (`DictionaryKey`, `Dictionary`).
- `crates/gup-core/src/encoding.rs`: `encode_owned_key` (line 338),
  `KeyEncoded::new` (line 761) and `domain` (line 811), `KeySource`/
  `KeyAccessor` and their impls (lines 826–925).
- The builders' `encode_owned_key` calls: `builder/scatter.rs` line 270,
  `line.rs` 275, `bar.rs` 683–752, `heatmap.rs` 208–213, `histogram.rs` 398.

## User Story

> "As a developer colouring a million-row chart by an owned key, I want the
> first draw to cost about what a borrowed key costs, so that the convenient API
> is not the slow one."

## Acceptance Criteria

### AC1: A committed benchmark (measure first)

- [ ] `crates/gup-core/examples/keyed_resolve_bench.rs` (copyright header), run
      with
      `scripts/with_cargo_env.sh cargo run -p gup-core --release --example keyed_resolve_bench`.
      It builds 1,000,000 rows `{ x: f64, y: f64, key: String }` with 8 distinct
      keys in a seeded pseudo-random order, and for each mode builds a fresh
      chart 5 times, timing only the first `resolve(&cx, 640.0, 400.0)`; it
      prints `metric keyed.<mode>.first_resolve_ms <median>` for modes
      `borrowed` (`Plot` with
      `Categorical::okabe_ito().encode_key(|r: &Row| r.key.as_str())` on
      `Circle::FILL`), `owned_plot` (the same with
      `encode_owned_key(|r: &Row| r.key.clone())`) and `builder`
      (`gup_core::scatter(rows).x(..).y(..).color(|r| r.key.clone())`).
- [ ] It also prints `metric keyed.dict.<path>.ns_per_row <v>` for 1,000,000 key
      encodes without the GPU: `borrowed` (`encode_borrowed(&str)`), `owned`
      (`Dictionary<String>::encode` of a clone), and `builder` (the builder's
      own path, as it is at each commit).
- [ ] The Implementation Summary records every metric at the parent commit (the
      example applied there uncommitted) and after.

### AC2: No second allocation on the builder path (change F1)

- [ ] The builders encode keys through a crate-private `LabelKey` accessor
      (Decisions) that hands the dictionary a borrowed `&str`; a key is copied
      into the dictionary only when it is new.
- [ ] Unit tests in `builder/mod.rs`: `Key::label` and `Key::visit` give the
      same text for a `String`, a `&'static str`, an `Arc<str>`, a `u32` and an
      enum with `Display`.

### AC3: A faster dictionary hash (change F2)

- [ ] `Dictionary`'s map uses `foldhash::fast::RandomState` (direct dependency
      `foldhash = "0.2"`, already in `Cargo.lock` through wgpu's hashbrown).

### AC4: Keep only what helps, and the target

- [ ] Each of F1 and F2 is kept only if, alone on top of the other, it lowers
      `keyed.builder.first_resolve_ms` by at least 5%; a change that does not is
      reverted and its numbers recorded.
- [ ] Target: `builder − borrowed` at most 30 ms (30 ns per row, from 52), both
      measured in the same run after the change, and `builder` at least 20%
      below its parent value.
- [ ] Behaviour unchanged: every test passes, including GUP-537's owned-key
      tests and the `builder_owned_keys` golden, with no golden re-blessed.

### AC5: Size

- [ ] `mask wasm-size`: the scatter gz bytes are reported; growth above 1,000 B
      is an escalation.

## Decisions

- **F1, `Key<T>` becomes a struct** in `builder/mod.rs`:

  ```rust
  pub(crate) struct Key<T>(Arc<dyn Fn(&T, &mut dyn FnMut(&str)) + Send + Sync>);
  impl<T> Key<T> {
      /// Call `f` with the row's label: no allocation for `String`,
      /// `&'static str` and `Arc<str>` keys; anything else is formatted
      /// into a reused thread-local `String`.
      pub(crate) fn visit(&self, row: &T, f: &mut dyn FnMut(&str));
      /// The label as an `Arc<str>` (allocates), for the builders' own CPU
      /// passes (stack sums, categories, label rows).
      pub(crate) fn label(&self, row: &T) -> Arc<str>;
  }
  impl<T> Clone for Key<T> { /* clones the Arc */ }
  ```

  `erase_key` keeps its signature and builds this. The builders' `xk(r)` calls
  become `xk.label(r)`.

- **The accessor**, in `encoding.rs`:

  ```rust
  pub(crate) struct LabelKey<T>(pub(crate) crate::builder::Key<T>);
  impl<T: 'static> KeySource for LabelKey<T> { type Key = Arc<str>; }
  impl<T: 'static> KeyAccessor<T> for LabelKey<T> {
      // visit the label; `dictionary.encode_borrowed(label)`; propagate the
      // error through a captured `Result`.
  }
  ```

  `KeyEncoded::new` becomes `pub(crate)`, and every builder call of
  `encode_owned_key(move |r: &T| key(r))` becomes
  `KeyEncoded::new(LabelKey(key.clone()), scale.clone())`, keeping its
  `.domain(..)` calls. Keys are still compared by label (GUP-537's rule), so
  nothing visible changes.

- **F2**: `codes: HashMap<K, u32, foldhash::fast::RandomState>`. It is seeded
  per process, so key order cannot be used to force collisions cheaply; codes
  come from first-seen order, never from hash order, so output is unchanged.
- **Rejected**: a cache keyed by the label's hash (it hashes every row, which is
  the cost being cut, and GUP-537 measured a cache with no gain); interning user
  keys across charts (memory that is never freed).

## Escalation Triggers

- After F1 and F2 the target is missed: record the numbers and a `perf record`
  profile summary of the `builder` mode, and stop; do not try other schemes.
- `foldhash` does not build for `wasm32-unknown-unknown` or AC5's growth is
  above 1,000 B.
- A test or golden changes behaviour.

## Out of Scope

- The user's own clone (inherent to owned keys).
- Plot-level `encode_owned_key` callers other than the builders (they keep
  `OwnedKey`; F2 speeds them up anyway).
- Columnar input that skips per-row closures (N5's larger story).

## Technical Tasks

- [ ] AC1 benchmark; run it at the parent commit and record.
- [ ] F1: `Key<T>` struct, `LabelKey`, builder call sites; tests.
- [ ] F2: `foldhash` dependency and the map's hasher.
- [ ] Measure each change alone and together (AC4); revert what does not help.
- [ ] `mask wasm-size`.

## Dependencies

### Prerequisite Stories

- GUP-537: Owned Category Keys in Builders ✅ — the API and the measured cost.

### Enables Stories

None directly.

## Testing Strategy

- **Unit tests**: `Key::visit`/`label` agreement (AC2).
- **Integration tests**: the existing builder and owned-key tests and goldens.
- **Performance**: AC1 benchmark, release, median of 5, before and after.

## Success Metrics

- [ ] Owned-key overhead at most 30 ns per row (from 52).
- [ ] No visible change.

## Risk Assessment

- **Medium**: machine noise can swamp a 5% rule. _Mitigation_: run each mode 5
  times and compare medians; repeat the A/B once if the two are within 5% of
  each other, and record both runs.
- **Low**: the thread-local buffer for formatted keys is borrowed while the
  key's `Display` (user code) runs. _Mitigation_: take it with
  `RefCell::try_borrow_mut` and fall back to `key.to_string()` when it is
  already borrowed, so a re-entrant `Display` cannot panic.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`, plus
      `scripts/with_cargo_env.sh cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not needed, no output change (goldens
      pass unchanged)
- [ ] `mask wasm-browser` not needed: no layout, input or wasm code changes
      beyond the hasher, which `mask wasm-size` builds
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported (AC5)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
