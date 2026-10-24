# Implementation Strategy

How Gup is built, what has been built, and what is next. The architecture itself
is in [TECHNICAL_APPROACH.md](./TECHNICAL_APPROACH.md). The reasons for the
current plan are in the
[strategic review](./planning/STRATEGIC_REVIEW_2026-10.md) (its "North star",
"Strategy" and "Decisions" sections); the design log is
[RFC-001](./planning/rfcs/RFC-001_Core_Architecture.md). This document has no
milestones, dates or estimates: the stories index records status, and a strategy
that carries dates is out of date by the next story.

## North star

A user writes about five to ten lines and gets a publication-quality chart,
identical in a window, a PNG, an SVG and WASM, with data encodings evaluated on
the GPU, and can drop from that chart into the low-level `Selection` API without
starting over. The code below is that sentence, and it is a doctest, so it stays
true.

```rust
use gup::prelude::*;

struct Row { gdp: f64, life_exp: f64, continent: String, population: f64 }
let data = vec![
    Row { gdp: 1e3, life_exp: 55.0, continent: "Africa".into(), population: 3e7 },
    Row { gdp: 6e4, life_exp: 81.0, continent: "Asia".into(), population: 1e9 },
];

let mut chart = gup::scatter(data)
    .x(|d| d.gdp)
    .y(|d| d.life_exp)
    .color(|d| d.continent.as_str())
    .size(|d| d.population);
let image = chart.render(&Context::shared()?, 800, 500)?;
assert_eq!(image.dimensions(), (800, 500));

// Drop down a level: the plot and its typed layers are ordinary values.
let mut plot = chart.into_plot();
plot.title("Wealth and health");
assert_eq!(plot.title_text(), Some("Wealth and health"));
# Ok::<(), gup::Error>(())
```

## How work is done

- **Stories.** All work is a story in `docs/planning/stories/`, with acceptance
  criteria, decisions, escalation triggers and a definition of done. Each story
  is small enough for one implementer to finish and review. The index
  (`docs/planning/stories/INDEX.md`) is the only list of status.
- **Fix the class, not the instance.** When a bug is fixed in one builder, mark
  or export path, every sibling with the same bug is fixed in the same story,
  unless the siblings are genuinely large, and then a follow-up is written.
- **Acceptance is what a user sees.** For anything visual the implementer
  renders a PNG and looks at it, and a golden image persists the check. "Not
  blank" is not acceptance.
- **Dogfood.** The detached `dogfood/` crate uses only the public `gup` API for
  realistic tasks. Gaps it exposes are tracked there, not remembered.
- **Gates are never bypassed.** The pre-commit hook, `mask all-check` and CI
  (lint, tests, visual regression, WASM size, the browser smoke test, dogfood)
  decide what merges. No `--no-verify`, no `#[ignore]`, no loosened tolerance. A
  budget is re-recorded only with a measured reason, in the same commit as the
  change. A known-broken output is a tracked entry in
  `tests/visual_regression/expected_failures.toml` that names the story that
  will fix it.
- **Breaking changes are allowed** (pre-alpha). A wrong API is replaced or
  deleted, never kept beside its replacement, and nothing is left as a silent
  no-op.
- **Orchestrator and implementers.** An orchestrator plans the sequence, writes
  and sizes stories and merges. Implementers (agents, in
  `.github/agents/story-worker.md`) take one story each, stop and report when an
  acceptance criterion cannot be met as written, and write a retrospective that
  feeds the next story.
- **Measure before claiming.** Performance and size claims come from
  `crates/gup-core/PERF_BUDGETS.md`, and counts of uploads and draws are exact
  and deterministic.

## What has been built

RFC-001 steps S0 to S14 (GUP-395 to GUP-489) are implemented. In one paragraph:
one cloneable `Context`; a chunked GPU column store with relative origins,
validity bits, dictionaries, `Retain`, append and `Window`; typed channels and
`#[derive(Mark)]` with the built-in `Circle`, `Rect`, `Segment` and `Line`; one
scale family in which each scale is a `ShaderFn` with an exact f64 `CpuMirror`;
build-time shader composition and `#[wgsl_function]`; a resolved `Scene` drawn
by image, window, texture and canvas targets and written as SVG; the `Chart`
trait with `Plot` as the universal container; layout, guides, legends, themes,
CPU and GPU picking, tooltips, pan, zoom, brush and culling; the `scatter`,
`line` and `bar` builders; and the egui and Bevy integrations. S14 made `gup` a
facade over `gup-core` and deleted the old render path (208,932 lines in the
flip). Each step's decisions and measurements are in RFC-001's findings
sections: see
[S14](./planning/rfcs/RFC-001_Core_Architecture.md#s14-findings-gup-485488) for
what the flip deleted and kept.

## What is next

The tracks below are defined in the
[November 2026 strategic review](./planning/STRATEGIC_REVIEW_2026-11.md)
("Proposed next tracks" and "Owner decisions"); the stories index holds their
stories and status.

- **N0, hygiene.** Current crate docs, stale docs archived, the mobile and Tauri
  crates deleted, honest gates and a checkpoint script for work in progress.
- **N1, docs and tutorials.** The six tutorials rewritten as tested Markdown, a
  `#[derive(Mark)]` guide and documented public items.
- **N2, chart vocabulary.** A text mark, area and stacked area, horizontal bars,
  histograms, a heatmap builder, line curves, a subtitle and a dark theme.
- **N3, release 0.1.** Publishable crates, CI on Windows, a gallery, a changelog
  and minimal accessibility: `Plot::describe()`, an SVG title and description
  and a canvas `aria-label`.
- **N4, accessibility.** Keyboard navigation, a hidden data table on the web and
  a high-contrast theme.
- **N5, performance at scale.** Columnar input, level of detail from
  `crates/gup-culling-lod`, and measured 100M- and 1B-row demonstrations.
- **N6, export and platforms.** PDF through SVG, and mobile or Tauri hosts when
  a user asks.

Parked items stay parked until a strategic review's decisions say otherwise; a
story that extends one needs a decision first.
