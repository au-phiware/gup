---
implementer: sonnet
implementer-reason:
  two setters and a per-layer choice of y scale threaded through the four places
  plot.rs hands text layers a scale, each named in Decisions; tests copy
  GUP-542's and GUP-555's
touches:
  [
    crates/gup-core/src/text_layer.rs,
    crates/gup-core/src/plot.rs,
    crates/gup-core/tests/text_layer.rs,
    crates/gup-core/tests/secondary_y.rs,
    tests/golden/gup_core/secondary_y.png,
  ]
---

# GUP-526: Text Layers on the Right Y Axis

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-26 (decision-complete rewrite 2026-10-31)

## Context

GUP-542's `TextLayer` always maps its y through the plot's left y scale. In
`plot.rs`, `layer.runs(&text, &**x, &**y, plot, &th)` is called around
line 2632. A dual-axis chart (`Plot::y_right`) draws a series against the right
axis, but a label for that series lands where the left scale puts the same
value. Nothing reports the error. A `Selection` binds to the right axis by
encoding through the right scale's handle (`right.encode(..)`). A text layer
takes plain closures, so it needs its own way to say which axis.

GUP-555 made text layers reserve room at the plot edges. It reports each layer's
reach and per-label edges along `[x, y]`, and `plot.rs` feeds them to the left y
axis only (`text_reach`, `label_room` and the `labels` candidates are
two-element arrays). A right-axis label must make room on the right axis
instead, or a label at the top of the right domain is clipped. Checked on main
on 2026-10-31: neither exists.

## Read First

- `crates/gup-core/src/text_layer.rs` lines 120–330 (`TextLayer` fields and
  setters, `y`, `y_key`, `within_y`), 394–418 (`check_axis`) and 440–630
  (`AnnotationLayer`: `runs`, `reach`, `edges`).
- `crates/gup-core/src/plot.rs`: `resolve_full`'s checks (lines 1866–1910); step
  5 from `let right_id` to the end of `solve_insets` (lines 2088–2320):
  `text_reach`, `text_over`, `label_room` and its `axes`, `widened`, and the
  `solve` block's `labels`; step 7's text-layer loop (lines 2626–2636).
- `crates/gup-core/tests/text_layer.rs`: the GUP-555 edge tests and the helper
  plots they build.
- `crates/gup-core/tests/secondary_y.rs` (the dual-axis golden, lines 1–190).

## User Story

> "As a visualization developer with a two-axis chart, I want to label the
> series drawn against the right axis, so that its labels sit on its points."

## Acceptance Criteria

### AC1: The API

- [ ] `TextLayer::y_right(f)` and `TextLayer::y_right_key(key)` place labels
      through the plot's right y scale, like `y` and `y_key` on the left. The
      later of `y`, `y_key`, `y_right` and `y_right_key` wins. `within_y` spans
      are read on whichever axis y is on. Each setter has rustdoc, and `y_right`
      has a doctest: a two-axis plot whose label sits at `right.eval(v)` (± 0.5
      px) after resolve.
- [ ] A layer on the right axis of a plot without one fails at resolve with
      `Error::config("text layer position", ..)`, whose message names
      `Plot::y_right`. Test in `tests/text_layer.rs`.
- [ ] `y_right(..)` on a band or point right axis, and `y_right_key(..)` on a
      numeric one, give `check_axis`'s errors naming `y_right_key(..)` and
      `y_right(..)` respectively. Test in `tests/text_layer.rs`.

### AC2: Placement

- [ ] Left `Linear` 0–10, right `Linear` 0–1000, a right-axis label at 500 with
      `dy(-6.0)`: its `PlacedText::run.at.y` equals `right.eval(500) - 6` within
      0.5 px. It is at least 20 px from the left scale's `eval(500) - 6`.
- [ ] Seeded failure, recorded in the retrospective: passing the left scale to
      `runs` for every layer makes this test fail.

### AC3: Room at the edge (GUP-555's rule on the right axis)

- [ ] Explicit right domain 0–1000, a label at 1000 anchored `BOTTOM`: its
      `PlacedText::bounds` lies inside `layout.plot`, so the label is drawn
      whole.
- [ ] Automatic right domain, a label at the data maximum anchored `BOTTOM`:
      drawn whole, and the right axis widened (its top tick is at or above the
      data maximum).
- [ ] With only left-axis labels, step 5 gives the same left and right insets as
      before. All of GUP-555's tests in `tests/text_layer.rs` pass unchanged.

### AC4: Golden (user-visible)

- [ ] The golden test in `secondary_y.rs`, and not `fixture()`, which other
      tests share, adds
      `plot.text(months()).x(|m| m.n).y_right(|m| m.rain).text(|m| m.rain).anchor(Anchor::BOTTOM).dy(-4.0)`.
      The rainfall bars are the right-axis series. Re-bless only
      `secondary_y.png` and read it. Each value sits just above its bar's top,
      the 180 mm label is whole (the automatic right axis widened), and the
      temperature line still follows the left axis's ticks.
- [ ] `git diff --stat tests/golden` lists only `secondary_y.png`.
- [ ] `mask wasm-browser` passes (step 5 changed).

## Decisions

- **Setters** (`text_layer.rs`), mirroring `y` and `y_key`:

  ```rust
  pub fn y_right<D: ColumnValue>(&mut self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> &mut Self;
  pub fn y_right_key<K: DictionaryKey>(&mut self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> &mut Self;
  ```

  They set `self.y` as `y` and `y_key` do, plus a new field `y_side: YSide`
  (`enum YSide { Left, Right }`, private, default `Left`). `y` and `y_key` set
  `Left`. We chose this over `y_axis(YAxis::Right)`: the name matches
  `Plot::y_right`, and a newcomer who has written `.y(|r| r.temp)` guesses
  `.y_right(|r| r.rain)`.

- **Trait**: `AnnotationLayer` gains `fn on_right_axis(&self) -> bool` (not
  `y_right`, which would share a name with the setter). `runs`, `reach` and
  `edges` keep their signatures. The plot passes the right scale as their `y`
  argument for a right-axis layer. Inside `runs`, `check_axis(name, ..)` gets
  `"y_right"` when `y_side` is `Right`, so its messages name `y_right_key(..)` /
  `y_right(..)`.
- **The missing-axis error** is raised in `resolve_full` beside the z-order
  check, before step 5:

  ```rust
  Error::config(
      "text layer position",
      format!(
          "text layer {i} places its labels on the right y axis (`y_right(..)`), \
           but the plot has none; add one with `Plot::y_right(scale)`"
      ),
  )
  ```

- **Step 5** (`plot.rs`). Index 2 is the right axis everywhere, as in
  `mark_over`.
  - `text_reach` becomes `[f32; 3]`. A layer's `reach()[1]` goes to index 1, or
    to index 2 when `layer.on_right_axis()`.
  - `text_over[2]` is `text_reach[2].min(0.25 * lens[2])`, or 0.0 when the right
    axis reads keys or is absent.
  - `label_room` becomes `[(f32, f32); 3]`. The automatic-axis `axes` array
    gains the right scale, with `ends[2] = (bottom, top)` and the same
    `EdgeAxis` construction. Each layer calls
    `edges(&**x, y_of(layer), [axes[0], axes[1 or 2]], &mut tmp)` into a fresh
    `[Vec<Edge>; 2]`. Then `tmp[0]` extends `cands[0]` and `tmp[1]` extends
    `cands[1]` or `cands[2]`. The loop solving `label_room` runs over all three
    axes.
  - `widened[2]` uses `padded(mark_over[2].max(label_room[2].0))` and `….1` in
    place of the symmetric `padded(mark_over[2])`. It is identical when there
    are no right-axis labels.
  - The explicit `solve` block collects labels the same way into `cands[2]`.
  - `y_of(layer)` is
    `if layer.on_right_axis() { right.expect("checked in resolve_full") } else { y }`.
- **Step 7**: `layer.runs(&text, &**x, y_of(layer), plot, &th)`.
- **No change** to `Selection`, scales or `inset.rs`.

## Escalation Triggers

- Feeding right-axis edges into the explicit solve changes any left-axis inset
  in an existing test.
- GUP-555's arrays turn out to be indexed in a way that assumes two axes beyond
  the places named above (for example in `inset::edge_insets` callers outside
  step 5).
- The `secondary_y` golden needs more than the new labels to change.

## Out of Scope

- Text layers placed on a second x axis: there is none.
- Label halos and overlap culling: GUP-527.

## Technical Tasks

- [ ] `YSide`, the setters and their docs and doctest;
      `AnnotationLayer::on_right_axis`.
- [ ] `resolve_full`'s missing-axis error.
- [ ] Step 5's three-axis arrays and step 7's scale choice.
- [ ] Tests AC1–AC3 in `tests/text_layer.rs`; the seeded failure.
- [ ] The `secondary_y` golden; `mask wasm-browser`.

## Dependencies

### Prerequisite Stories

- GUP-542 ✅: the text layer.
- GUP-555 ✅: label room at the plot edge (the arrays this extends).

## Testing Strategy

- **Unit tests**: none new; `text_layer::tests` must pass.
- **Integration tests**: AC1–AC3 in `crates/gup-core/tests/text_layer.rs`.
- **Visual validation**: `secondary_y.png` read by eye.
- **Performance**: unchanged (one more array slot).

## Success Metrics

- [ ] A dual-axis chart labels both of its series with two `plot.text(..)`
      calls.

## Risk Assessment

- **Medium**: step 5 is dense, and GUP-555's arrays are easy to index wrongly.
  _Mitigation_: the edge tests for the left axis must pass unchanged, and AC3
  covers both right-axis policies.
- **Low**: `plot.rs` is a hot file. _Mitigation_: `touches` names it, so no
  other `plot.rs` story runs beside this one.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (`secondary_y.png`)
- [ ] `mask wasm-browser` (step 5 changed)
- [ ] WASM size: `mask wasm-size` scatter gz bytes (text layers are generic;
      expect at most 1,000 B gz)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
