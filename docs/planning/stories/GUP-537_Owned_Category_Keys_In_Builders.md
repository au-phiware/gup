---
implementer: sonnet
implementer-reason:
  signatures, key erasure and both diagnostics are fixed in Decisions and were
  compile-checked against gup-core; the rest is migrating named call sites
touches:
  [
    crates/gup-core/src/builder/mod.rs,
    crates/gup-core/src/builder/bar.rs,
    crates/gup-core/src/builder/line.rs,
    crates/gup-core/src/builder/scatter.rs,
    src/lib.rs,
    dogfood/,
  ]
---

# GUP-537: Owned Category Keys in Builders

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 🚧 In
Progress Planned **Created**: 2026-10-24

## Context

Every builder setter that takes a category key (`ScatterChart::color`,
`LineChart::color`, and `BarChart::x`, `color`, `group` and `stack`) takes
`impl for<'a> Fn(&'a T) -> &'a str`. That accepts `|r| r.name.as_str()` and
`&'static str` fields, but an integer year, an enum, or any other key a real
user has gets "mismatched types: expected `&str`". The user has to write a
`fn as_str` for an enum, or keep a `String` copy of the year. The N2 scribe
found that one method cannot take both forms. With
`impl Fn(&T) -> K, K: DictionaryKey`, rustc cannot express a return type that
borrows from the argument, so `|r| r.name.as_str()` fails. With a custom `KeyFn`
trait implemented for both closure shapes, the closure's signature is no longer
inferred, so every call needs annotations ("type annotations needed"). N2
deferred this as an owner decision.

The owner decided (2026-10-24, final): builders take **owned** keys. There is
one method name per setter, and closures return owned values: `|r| r.year`,
`|r| r.region` (a `Copy` enum with `Display`), `|r| r.name.clone()`, a
`&'static str` field or literal. One rule a user can guess beats two method
names. Accessors run once per row at upload, so a clone costs once per row.
RFC-001's original north-star example already wrote
`.color(|d: &Row| d.continent.clone())` (RFC-001 line 681).

The Plot layer already supports owned keys. `EncodeFn::encode_owned_key` takes
`Fn(&T) -> K` with `K: DictionaryKey` (any
`Hash + Eq + Clone + Display + Send + Sync + 'static` type,
`crates/gup-core/src/column.rs` line 150), and `encode_key` keeps the borrowed,
allocation-free `&str` path (RFC-001 "S5b findings"). This story changes only
the builders. The Plot-layer entry points stay as they are.

The planner ran throwaway compiles against gup-core at `5aa7fe68` (bar.rs
switched to the Decisions below, then reverted). `|r| r.year`, a `Copy` enum,
`r.name.clone()`, a `&'static str` field and a literal all compile with no
annotations. `|r| r.name.as_str()` fails with rustc's region error, "lifetime
may not live long enough … returning this value requires that `'1` must outlive
`'2`", at the closure. A custom message is **not achievable** for that case.
`#[diagnostic::on_unimplemented]` fires only on an unmet trait bound, and type
checking erases lifetimes. So `&'a str` meets `DictionaryKey` exactly as
`&'static str` does, and the error comes later, from the borrow checker, which
has no diagnostic hook. Rejecting every `&str` at type check would give the
custom message, but it would also reject `&'static str` literals and fields. It
would also need a hand-written impl list instead of the blanket impl, so a user
enum would need its own `impl`. That breaks the owner's rule, so the story takes
the best achievable outcome:

- A borrowed return: rustc's lifetime error pointing at the closure's return
  expression. It is snapshotted, and every key setter's docs say "return an
  owned key, e.g. `r.name.clone()`".
- A type that is not a key (`f64`, an enum without `Display`): a custom
  `on_unimplemented` message on `DictionaryKey`, also snapshotted, that names
  the owned forms.

## Read First

- `crates/gup-core/src/builder/bar.rs`: lines 15–37 (`Key`, `key_fn`,
  `ColorSpec`, `Arrange`), 148–222 (the key setters), 250–392 (`split`,
  `materialise`).
- `crates/gup-core/src/builder/line.rs`: lines 17–29, 158–164 (`color`), 233–335
  (`split`, `add_series`, `materialise`, `drain_feed`).
- `crates/gup-core/src/builder/scatter.rs`: lines 14–29, 126–132 (`color`),
  250–270 (`materialise`).
- `crates/gup-core/src/builder/mod.rs`: the module docs (lines 1–90, including
  the migration table).
- `crates/gup-core/src/column.rs` lines 146–152 (`DictionaryKey`).
- `crates/gup-core/src/encoding.rs` lines 260–345 (`encode_key`,
  `encode_owned_key`; used, not changed) and 760–830 (`KeyEncoded::new`, the
  shared-scale dictionary type check, `domain`).
- `crates/gup-core/tests/compile_fail.rs` (module docs) and
  `tests/compile_fail/borrowing_accessor.{rs,stderr}` (the snapshot style).
- `crates/gup-core/tests/builders.rs` lines 600–760 (the byte-identical bar
  tests) and 895–950 (the `harness().run` golden pattern).
- RFC-001 "S5b findings", "The dictionary hook" (lines 2668–2690).

## User Story

> "As a visualization developer, I want to pass `|r| r.year`, `|r| r.region` or
> `|r| r.name.clone()` to any builder's category setter, so that I don't have to
> turn my keys into borrowed strings first."

## Acceptance Criteria

### AC1: Owned-key signatures on every builder key setter

- [ ] The six setters have exactly the signatures in **Decisions**, and no
      builder setter takes `for<'a> Fn(&'a T) -> &'a str` any more
      (`git grep -n "for<'a> Fn(&'a T) -> &'a str" -- crates/gup-core/src/builder`
      prints nothing).
- [ ] `key_fn` is gone from all three builder files. The single erasure helper
      lives in `builder/mod.rs`.
- [ ] New `tests/compile_pass/builder_owned_keys.rs` compiles. For every one of
      the six setters, it passes an `i32` field, a `Copy` enum with `Display`, a
      `String` via `.clone()`, a `&'static str` field and a string literal, with
      no closure annotations and no turbofish.
- [ ] `tests/compile_pass/builder_scatter.rs` uses
      `.color(|d| d.continent.clone())` and still passes.

### AC2: Diagnostics

- [ ] `DictionaryKey` carries the `#[diagnostic::on_unimplemented]` attribute
      exactly as given in **Decisions**.
- [ ] New `tests/compile_fail/builder_borrowed_key.rs` (bar
      `.x(|r| r.name.as_str())`) and its `.stderr` snapshot. The snapshot is one
      error, `error: lifetime may not live long enough`, whose primary span is
      `r.name.as_str()`, with the label "returning this value requires that `'1`
      must outlive `'2`". The `.rs` file's header comment explains why rustc
      gives no custom message (see Context).
- [ ] New `tests/compile_fail/builder_key_without_display.rs` (bar
      `.x(|r| r.region)`, where `Region` derives
      `Clone, Copy, Hash, PartialEq,     Eq` but has no `Display`) and its
      `.stderr`. The snapshot starts with
      ``error[E0277]: `Region` is not a category key``, labels the `x` call
      "this key accessor returns `Region`", and contains the note from
      Decisions.
- [ ] Every other `.stderr` under `tests/compile_fail/`,
      `tests/wgsl_function_fail/` and `tests/derive_mark_fail/` is unchanged
      (`git diff --stat` on those directories shows only the two new pairs).
- [ ] The module docs of `tests/compile_fail.rs` gain one sentence on what the
      two new snapshots cover.

### AC3: Behaviour (`tests/builders.rs`, `tests/append.rs`)

- [ ] The four byte-identical tests pass without edits to their reference plots,
      which are built with `encode_key`:
      `byte_identical_to_the_reference_scatter`,
      `line_is_byte_identical_to_the_reference_plot`,
      `simple_bars_are_byte_identical_to_the_hand_built_plot` and
      `grouped_bars_are_byte_identical_to_four_hand_built_layers`. Only the
      builder side's key closures change (`.as_str()` to `.clone()`).
- [ ] New test
      `integer_and_enum_keys_label_bands_and_legend_in_first_seen_order`: `bar`
      over rows with years in the order 2021, 2019, 2020 (`i32`), grouped by a
      `Region { North, South }` enum with `Display`. In `layout.texts`, the tick
      labels "2021", "2019" and "2020" run left to right in that order (by
      `bounds.x`), and the legend holds "North" then "South".
- [ ] New test `keys_with_the_same_label_are_one_category`: two enum variants
      whose `Display` is the same string, used as bar `.x` keys, give one band:
      that label appears exactly once in `layout.texts`.
- [ ] New test in `tests/append.rs`: a `line` over rows with `u32` ids 7 and 9,
      keyed by `.color(|p| p.2)` (2 layers). After pushing a row with id 7
      through `appender()` and resolving, `layers_mut().len()` is still 2; after
      pushing id 11 and resolving, it is 3.

### AC4: Every call site migrated

- [ ] These call sites now return owned keys. For a `String` field that is
      `.clone()`; for t4's `if`, it is `r.pt.segment.clone()` /
      `"unselected".to_owned()`:
  - `crates/gup-core/src/builder/mod.rs` line 22 (module doctest) and the
    migration-table rows at lines 67 and 78 (`.color(\|d\| d.key.clone())`,
    `.group(\|d\| d.k.clone())` / `.stack(\|d\| d.k.clone())`);
  - `crates/gup-core/tests/builders.rs` lines 64, 175, 177, 285, 310, 413, 417,
    456 and 461;
  - `crates/gup-core/tests/compile_pass/builder_scatter.rs` line 20;
  - `src/lib.rs` line 26 (the `gup` facade's doctest);
  - `README.md` line 65, `docs/IMPLEMENTATION_STRATEGY.md` line 32,
    `docs/TECHNICAL_APPROACH.md` lines 25 and 363, `dogfood/README.md` line 94;
  - `dogfood/src/bin/t1_timeseries.rs` line 45; `t2_bars.rs` lines 43, 45, 50
    and 52; `t3_scatter_png.rs` line 23; `t3_scatter_window.rs` line 23;
    `t4_linked.rs` lines 47 and 92–98.
- [ ] Afterwards, this prints exactly two lines, both Plot-layer `encode_key`
      calls that stay: `tests/builders.rs` line 321 (approximately, after edits)
      and `tests/compile_pass/key_accessor.rs` line 22.

      ```bash
      git grep -n "as_str()" -- crates/gup-core/src/builder \
        crates/gup-core/tests/builders.rs crates/gup-core/tests/compile_pass \
        dogfood examples src/lib.rs README.md docs/IMPLEMENTATION_STRATEGY.md \
        docs/TECHNICAL_APPROACH.md
      ```

- [ ] Call sites that already return `&'static str` fields or `Copy` values
      (`examples/basic/03_line_chart.rs`, `04_bar_chart.rs`, the `bar.rs` and
      `line.rs` doctests, `tests/append.rs`, `tests/builders.rs` lines 343,
      568–577, 643, 665 and 865) compile unchanged.
- [ ] History documents stay as written:
      `docs/planning/STRATEGIC_REVIEW_2026-10.md`, RFC-001, archived stories.

### AC5: Docs

- [ ] `builder/mod.rs` gains a `# Category keys` section, with a running doctest
      as sketched in **Decisions** (an external-crate use of the new API), and a
      bullet under `# Pattern`: "Category keys are owned: see
      [Category keys](#category-keys)."
- [ ] Each of the six setters' docs ends with: "The accessor returns an owned
      key (an integer, an enum with `Display`, `r.name.clone()`, a
      `&'static str`); see [category keys](crate::builder#category-keys)."

### AC6: Visual

- [ ] New golden `gup_core/builder_owned_keys` (480×320) in `tests/builders.rs`,
      built like `bar_golden_has_gaps_a_legend_and_bars_on_the_axis`.
      `bar(..).x(|r| r.year).y(|r| r.total).group(|r| r.region).legend_title("Region")`,
      with years 2019, 2020 and 2021 (rows in that order) and two regions, and
      `with_expected_color` for each region's `OKABE_ITO[i]`. The worker reads
      the PNG and confirms: three bands labelled 2019, 2020 and 2021 left to
      right, two bars side by side per band in the two palette colours, the
      legend "Region" with North then South, and every bar inside the plot area.
- [ ] Every existing builder golden (`gup_core/builder_bars` and the line and
      scatter goldens) passes with no snapshot update.

### AC7: Cost

- [ ] In release mode, time the first `Chart::resolve` of a 1,000,000-row
      `scatter` coloured by 8 distinct `String` keys. Measure at the parent
      commit (`.color(|r| r.key.as_str())`) and after the change
      (`.color(|r| r.key.clone())`), median of 5 runs each, using a throwaway
      example that is not committed. Record both numbers in the Implementation
      Summary.

## Decisions

- **Signatures** (the bound is the existing public
  `gup_core::column::DictionaryKey`; no new trait, no new root or prelude
  export):

  ```rust
  impl<T: Send + Sync + 'static> ScatterChart<T> {
      pub fn color<K: DictionaryKey>(mut self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self;
  }
  impl<T: Send + Sync + 'static> LineChart<T> {
      pub fn color<K: DictionaryKey>(mut self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self;
  }
  impl<T: Send + Sync + 'static> BarChart<T> {
      pub fn x<K: DictionaryKey>(mut self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self;
      pub fn color<K: DictionaryKey>(mut self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self;
      pub fn group<K: DictionaryKey>(mut self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self;
      pub fn stack<K: DictionaryKey>(mut self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self;
  }
  ```

  Why `DictionaryKey`: it is already the bound of `encode_owned_key`, its docs
  already say "strings, integers, enums, anything hashable whose `Display` is
  its label", and a second trait with the same blanket impl would be two names
  for one rule.

- **User-facing before and after**:

  ```rust
  // Before (5aa7fe68): only borrowed strings.
  gup::bar(sales)
      .x(|s| s.name.as_str())
      .y(|s| s.total)
      .group(|s| s.region.as_str()); // an enum needed a hand-written `as_str`
  // `.x(|s| s.year)` was "mismatched types: expected `&str`, found `i32`".

  // After: owned keys, one method each.
  gup::bar(sales)
      .x(|s| s.year)          // i32
      .y(|s| s.total)
      .group(|s| s.region);   // Copy enum with Display
  gup::scatter(people).x(|p| p.age).y(|p| p.income).color(|p| p.team.clone()); // String
  gup::line(prices).x(|p| p.day).y(|p| p.close).color(|p| p.ticker);           // &'static str: unchanged
  ```

- **Erasure** (in `builder/mod.rs`, shared by every builder, including those N2
  adds later):

  ```rust
  /// A category key accessor, erased to the key's label.
  pub(crate) type Key<T> = Arc<dyn Fn(&T) -> Arc<str> + Send + Sync>;

  /// Erase an owned key accessor to one returning the key's label.
  pub(crate) fn erase_key<T, K: DictionaryKey>(
      key: impl Fn(&T) -> K + Send + Sync + 'static,
  ) -> Key<T>;

  /// A key's label: an `Arc<str>` key itself, a `String` or `&'static str`
  /// copied once into an `Arc<str>`, anything else through `to_string()`.
  pub(crate) fn label<K: DictionaryKey>(key: K) -> Arc<str>;
  ```

  `label` picks the three cases with `(&key as &dyn Any).downcast_ref::<..>()`
  (`K: 'static` holds). The builders' private `Key<T>` aliases and `key_fn` go.
  `ColorSpec::Key`, `Arrange::Group`/`Stack` and the line's key store
  `builder::Key<T>`.

- **Builders compare keys by label**: two keys whose `Display` is equal are one
  category, one colour and one series. This is documented in the
  `# Category keys` section. Why: it lets every builder channel stay an
  `Arc<str>` dictionary, the same type `encode_key` makes. A shared scale's
  dictionary is typed (`KeyEncoded::new` reports "layers share one scale but key
  it by different types"), so the builder's layers stay compatible with layers a
  user adds through `plot_mut()` with `encode_key`. The four byte-identical
  tests also stay byte-identical. On a chart, two keys that print the same could
  not be told apart anyway.

- **Encoding**: every builder channel encodes through
  `scale.encode_owned_key(move |r: &T| key(r))` (K = `Arc<str>`), replacing
  `encode_key(key_fn(..))`. For bar's x this is `self.x.encode_owned_key(..)`
  and `self.x.sub_band(i, count).encode_owned_key(..)`. The
  `.domain(categories.iter().cloned())` calls are unchanged.
- **Internal maps** switch to `Arc<str>` keys: bar's `split` index and
  `categories` set, the stack tables' lookups (`top.get(&xk(r))`), and the
  line's `split` index and `keys: HashMap<Arc<str>, usize>`.
- **The diagnostic** on `DictionaryKey` in `column.rs`, verbatim:

  ```rust
  #[diagnostic::on_unimplemented(
      message = "`{Self}` is not a category key",
      label = "this key accessor returns `{Self}`",
      note = "a category key is an owned value that is `Hash + Eq + Clone + Display`: an integer, a `String` (`|row| row.name.clone()`), an enum that implements `Display`, or a `&'static str`"
  )]
  pub trait DictionaryKey: Hash + Eq + Clone + Display + Send + Sync + 'static {}
  ```

  The planner checked this: for `f64` it prints the message twice (`Hash`, then
  `Eq`), each with rustc's std-type list. That is why the snapshot uses the enum
  without `Display`, which prints one clean error.

- **Plot-layer APIs stay unchanged**: `encode_key`, `encode_nullable_key`,
  `encode_owned_key`, `encode_nullable_owned_key`, `KeyEncoded::domain`,
  `Selection`'s adders, and `borrowing_accessor.stderr` (which still points at
  `encode_key`/`encode_owned_key`, both of which still exist). Why: the Plot
  layer is the explicit level, where the borrowed path saves a per-row
  allocation (RFC-001 S5b), and its users already pick a function per key kind.
- **The `# Category keys` doctest** (in `builder/mod.rs`, about 20 lines):
  `Sale { year: i32, region: Region, total: f64 }` with a `Region` enum that
  implements `Display`,
  `bar(rows).x(|s| s.year).y(|s| s.total).group(|s| s.region)`,
  `render_resolved`, and an assert that `layout.texts` contains "2021", "2022",
  "2023", "North" and "South". The prose says: owned keys, the four usual forms,
  "a closure returning `r.name.as_str()` does not compile (a lifetime error):
  return `r.name.clone()`", and keys compared by label.
- **Not in this story**: nullable builder keys (`Option<K>` is not `Display`, so
  it gets the new message). The builders other N2 stories add take keys by these
  same rules, with `erase_key`; their stories were updated when this story was
  written.

## Escalation Triggers

- A byte-identical test (AC3) fails, or passes only after its `encode_key`
  reference plot is changed.
- `builder_borrowed_key` gives anything but the single lifetime error at the
  closure: an E0277 on `&str`, "type annotations needed", or a second error.
- Adding the attribute to `DictionaryKey` changes any existing `.stderr`.
- A migrated call site needs a closure annotation (`|r: &Row|`) or a turbofish
  to compile.
- AC7's owned-key first resolve is more than 1.5× the borrowed-key one. The fix
  (a buffered label lookup through a new `KeyAccessor`) would touch
  `encoding.rs`, so stop and report the numbers.
- A dogfood check changes status (`./dogfood/run_all.sh`).

## Out of Scope

- Plot-layer key entry points (see Decisions); they stay.
- `barh` (GUP-543), `TextLayer::x_key`/`y_key` (GUP-542), and the area,
  histogram, heatmap and box plot builders (GUP-547, 548, 549 and 554). They
  adopt owned keys in their own stories, whose Decisions say so.
- Nullable keys in builders, and a builder-level category order setter (such as
  `x_domain`).
- The tutorial rewrites (GUP-530, GUP-536) and the README rewrite (GUP-534).
  This story edits only the call sites listed in AC4.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [ ] `builder/mod.rs`: `Key<T>`, `erase_key` and `label`; unit tests for
      `label` (an `Arc<str>` key returns the same pointer, `String` and `i32`
      give the expected text).
- [ ] Switch `scatter.rs`, `line.rs` and `bar.rs` to the new signatures,
      `erase_key` and `encode_owned_key`; delete `key_fn`; migrate the internal
      maps.
- [ ] Add the `DictionaryKey` attribute; migrate `tests/builders.rs` and
      `compile_pass/builder_scatter.rs`; run the byte-identical tests; commit.
- [ ] Add the compile-pass and compile-fail files. Generate the two `.stderr`
      files with `TRYBUILD=overwrite` (pinned toolchain) and read them against
      AC2; commit.
- [ ] Add the AC3 tests and the AC6 golden; read the PNG; commit.
- [ ] Write the docs (AC5) and migrate the AC4 call sites (dogfood, README,
      docs, `src/lib.rs`); run the AC4 grep; commit.
- [ ] AC7 measurement; Implementation Summary.

## Dependencies

### Prerequisite Stories

- GUP-419: dictionary hook, `encode_owned_key` and `DictionaryKey` ✅.
- GUP-462, GUP-463 and GUP-464: the scatter, line and bar builders ✅.
- GUP-523: the `borrowing_accessor` snapshot style ✅.

### Enables Stories

- GUP-543: `barh`'s `y` and the moved bar setters take owned keys.
- GUP-547, GUP-548, GUP-549 and GUP-554: new builders reuse `erase_key`.
- GUP-542: `TextLayer::x_key`/`y_key` follow the same rule.
- GUP-530, GUP-534 and GUP-536: tutorials and the README show the owned forms.

## Testing Strategy

- **Unit tests**: `label` in `builder/mod.rs`; the existing `stack_tables`
  tests, with `Arc<str>` keys unchanged.
- **Integration tests**: `tests/builders.rs` (byte-identical, order, label
  identity, golden), `tests/append.rs` (keyed line with `u32` ids),
  `tests/compile_fail.rs` (two new snapshots, two compile-pass files).
- **External crate**: the `# Category keys` doctest, the `gup` facade doctest in
  `src/lib.rs`, and the dogfood crate (`./dogfood/run_all.sh`), which depends on
  `gup` by path like a user.
- **Visual validation**: the `gup_core/builder_owned_keys` golden, read by eye.
- **Performance**: AC7, recorded only.

## Success Metrics

- [ ] `.x(|r| r.year)`, `.group(|r| r.region)` and `.color(|r| r.name.clone())`
      compile on every builder with no annotations.
- [ ] Zero builder key closures in the repository return `.as_str()` (AC4 grep).
- [ ] Four byte-identical tests and every existing golden pass with no reference
      change.

## Risk Assessment

- **Low**: the borrowed-key error is rustc's, not ours, and it could change
  wording with a toolchain bump. _Mitigation_: it is snapshotted like the
  others, and it names no gup internals, so only a toolchain bump moves it. The
  docs carry the advice the compiler cannot give.
- **Low**: per-row label allocation on large keyed charts. _Mitigation_: AC7
  measures it, `label` avoids the double copy for `String` and `Arc<str>`, and
  the escalation trigger stops before an `encoding.rs` redesign.
- **Low**: label identity merges two keys that print alike. _Mitigation_:
  documented and tested. For a chart, such keys could not be told apart anyway.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`, plus
      `scripts/with_cargo_env.sh cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the AC6 golden)
- [ ] `mask wasm-browser` not needed: no layout, input or wasm code changes (the
      browser scenes use no builder)
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported (expected unchanged:
      the WASM scenes use no builder, and the attribute adds no code)
- [ ] `./dogfood/run_all.sh` passes with every check's status unchanged
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
