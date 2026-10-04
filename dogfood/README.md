# Gup dogfood suite

Six realistic tasks a new user might try with Gup, written as an **external
crate**. The suite runs in CI and checks that each task produces the chart it
was meant to produce.

The tasks come from the October 2026 real-world-usage audit (see
`docs/planning/STRATEGIC_REVIEW_2026-10.md`). That audit completed 3 of 6 tasks,
and only with about 50% workaround code. This crate keeps those tasks, including
their workarounds and their failures, and runs them on every change. Gaps are
therefore tracked continuously instead of being recorded once.

## Why it is a detached crate

`Cargo.toml` declares its own empty `[workspace]` and depends on
`gup = { path = ".." }`. As a result it:

- is **not** a member of the parent workspace, so `cargo build` at the repo root
  does not build it, and it cannot see `pub(crate)` items;
- consumes `gup` exactly as a downstream user would. Every import here is public
  API, and every helper in `src/lib.rs` is code a user had to write because Gup
  doesn't provide it.

## Running it

```sh
./dogfood/run_all.sh                       # needs a display for 3 tasks
xvfb-run -a ./dogfood/run_all.sh           # headless, with Xvfb
DOGFOOD_SKIP_WINDOWED=1 ./dogfood/run_all.sh
DOGFOOD_ONLY=t1_timeseries,t2_bars ./dogfood/run_all.sh
mask dogfood                               # same as the first line
```

The script builds every binary in release mode and runs
`target/release/dogfood_check`. Outputs, CSV fixtures and per-task stdout/stderr
logs go to `/tmp/gup-dogfood/`. `DOGFOOD_TIMEOUT_SECS` sets the per-task timeout
(default 300).

## How results are classified

| Status  | Meaning                                                        | Fails the build |
| ------- | -------------------------------------------------------------- | --------------- |
| `PASS`  | The check met its bar.                                         | no              |
| `XFAIL` | A **tracked known gap** still fails, as expected.              | no              |
| `FAIL`  | An untracked failure: a regression, or a new gap.              | **yes**         |
| `XPASS` | A tracked gap now passes. Delete its `gap` entry in the suite. | **yes**         |
| `SKIP`  | Windowed task skipped via `DOGFOOD_SKIP_WINDOWED=1`.           | no              |

Never "fix" a gap by loosening its check. Fix Gup instead, watch the check
`XPASS`, then turn it into a plain check.

## Layout: intent separate from API calls

- `src/suite.rs` is the **manifest**. It holds each task's intent, the files it
  must write, the pixel checks on those files, and the known gaps. It contains
  no Gup API calls.
- `src/pixels.rs` holds Gup-independent measurements on decoded RGBA images
  (background, coverage, ink, hue, exact colour, grey, horizontal runs). It is
  deliberately small and should migrate to the shared visual-assertion module
  (GUP-388) once that lands.
- `src/bin/*.rs` are the tasks, written against today's `gup` API. When the new
  core (`crates/gup-core`, RFC-001) replaces that API, rewrite the task
  binaries. The manifest stays put, so the bar the task has to clear does not
  move. RFC-001 step S10's exit criterion is "dogfood tasks 1-3 pass".
- `src/bin/dogfood_check.rs` is the runner.

## The tasks

Gaps marked _(tracked)_ are encoded in `src/suite.rs` and checked on every run.
The others are recorded here, from the task sources, but have no pixel check.

### Control and reference

- **`t0_smoke`**: the simplest scatter (20 points) with a title and axes,
  exported to PNG. Points and axis lines render. The title and tick labels are
  missing _(tracked: no text in PNG)_. Circles render as ellipses on the 800x500
  canvas. The audit also tried the README-style `plot().scatter(x("x"), y("y"))`
  route. It "succeeded" but drew every point on one spot, because field
  accessors evaluate to `0.0`. GUP-389 deletes that API, so the suite no longer
  runs it.
- **`ref_export_png`**: Gup's own `examples/export_png.rs` (a raw `Selection`
  inside a `ComposedChart`, with title, subtitle and grid), copied and built as
  an external crate, so the reference path is checked from outside too. Steel
  blue points render, but at a lighter colour than configured _(tracked: double
  gamma)_. Title missing _(tracked)_. Grid missing _(tracked)_. The point at x=1
  lands left of the y axis because the selection draws in whole-canvas clip
  space while the axes use the plot rect _(tracked)_.

### Task 1: `t1_timeseries`

Multi-line time series (three tickers, five years of daily prices with gaps)
from CSV, with a date axis, legend and title, exported to PNG.

- Works: one line per ticker via a categorical colour accessor, and gaps via
  `connect_nulls(false)`.
- User workarounds in the source: domains computed by hand (axes default to
  0..1), and timestamps in **milliseconds** because that is what
  `DateTimeFormatter` assumes (undocumented). The plot rect is recovered by
  parsing Gup's own SVG output.
- Gaps: PNG has no title, tick labels or legend _(tracked)_. There is no legend
  API _(tracked)_. Series colours are desaturated _(tracked: double gamma)_. The
  configured horizontal grid is not drawn _(tracked)_.
- Workaround output `t1_svg.png`: Gup's SVG export has axes and text but no data
  marks _(see task 3)_. So the task hand-builds the line paths and legend as
  `SvgElement`s in pixel space and rasterises them with ImageMagick (`magick` or
  `convert`). The suite checks that this output has a title, date ticks and
  series.

### Task 2: `t2_bars`

A grouped and a stacked bar chart (4 regions x 4 quarters) with legend and value
labels, exported to PNG.

- Part A uses the documented `group_by` / `stack_by` API. `group_by` changes
  nothing: one bar per quarter, overdrawn _(tracked)_. `stack_by` does not stack
  _(tracked)_. `.color()` with a `String` value renders grey instead of using a
  palette _(tracked)_. No title _(tracked)_. No legend API _(tracked)_.
- Part B is the workaround. It encodes (quarter, region) into one band key for
  grouping and draws overlapping cumulative bars, tallest first, for stacking,
  with explicit `AccessorValue::Color`. It re-derives the band layout from the
  SVG axes to place value labels and a legend in SVG. The PNG has 16 grouped
  bars and 4 stacks in the right hues, though not the exact configured colours
  _(tracked: double gamma)_.

### Task 3: `t3_scatter_png` and `t3_scatter_window`

A 200k-point scatter: colour = categorical segment, size = numeric weight, log x
axis.

- `t3_scatter_png`: renders all five segments. The task works around: a
  categorical colour rendering black (so it maps colours by hand), size being
  "percent of plot width" (undocumented), and the circle's default black stroke
  drowning small points. For the last, it reaches into the public
  `visualization` field to zero `stroke_width` and re-prepare. Gaps: no title or
  tick labels _(tracked)_, no grid _(tracked)_, and the SVG export contains no
  data points _(tracked)_.
- `t3_scatter_window`: the same chart in an `eframe` window with a hover
  tooltip. `GupApp` has no input hooks, `ComposedChart`'s hover reveal only
  reveals clipped label text, and `gup-egui` didn't compile at audit time. So
  the task embeds the chart via `render_to_rgba` into an egui texture
  (`ChartTexture` in `src/lib.rs`), recomputes plot-rect fractions that Gup
  keeps `pub(crate)` (`plot_frac`), and does its own nearest-point search. The
  egui-drawn title and tooltip work. The chart texture has no tick labels
  _(tracked)_.

### Task 4: `t4_linked`

Two linked scatter views (30k points) in egui. Brushing the left view highlights
the same customers in the right view, linked through Gup's
`SharedSelectionState`. This **works**. The brush query and the highlight
re-binding (a colour accessor closing over the shared state, re-evaluated by
`prepare_render_bound`) are user code, as is the egui embedding.

### Task 5: `t5_live` and `t5_stream`

Append points every 100 ms without rebuilding the chart.

- `t5_live`: a line chart (random walk) and a 100k-point scatter. This
  **works**, but there is no append API. Every tick copies all line segments or
  all points and calls `set_data` + `prepare_render_bound`, re-uploading
  everything. The scatter costs tens of milliseconds per tick.
- `t5_stream`: Tutorial 5's `DataStream` -> `Selection::stream` route. The
  stream accepts batches, but the selection never becomes render-ready:
  `prepare_render_bound` fails with "No attribute bindings set", and every PNG
  is blank _(tracked)_.

### Task 6: `t6_wgsl`

A custom `#[wgsl_function]` GPU transform (a soft-knee radius) in a downstream
crate, wired through `attr_shader`, following Tutorial 3.

- Since GUP-377, the macro compiles in an external crate with no workarounds.
  The audit's `use gup::*;` glob and direct `bytemuck` dependency are gone.
- `t6_wgsl_tutorial` (`RAW_MACRO=1`) uses the macro output exactly as the
  tutorial shows. It still **panics**: wgpu reports "redefinition of
  `KneeRadiusUniforms`", because the macro's WGSL embeds the uniforms struct and
  `Selection`'s shader assembly emits it again _(tracked expected failure)_.
- `t6_wgsl_workaround` wraps the generated type and strips the struct from its
  WGSL. Point radii then grow left to right as intended. Colours are not the
  exact configured values _(tracked: double gamma)_.

## Fixtures

`gen_data` writes `prices.csv`, `sales.csv` and `points.csv` (200k rows) to
`/tmp/gup-dogfood/` from a fixed-seed generator, so outputs are reproducible. It
has no Gup dependency.
