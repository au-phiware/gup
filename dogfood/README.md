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
`gup = { path = ".." }` (the old API, for the tasks that have not moved) and
`gup-core = { path = "../crates/gup-core" }` (the new core, until RFC-001 S14
renames it to `gup`). As a result it:

- is **not** a member of the parent workspace, so `cargo build` at the repo root
  does not build it, and it cannot see `pub(crate)` items;
- consumes `gup` exactly as a downstream user would. Every import here is public
  API, and every helper in `src/lib.rs` is code a user had to write because Gup
  doesn't provide it.

## Running it

```sh
./dogfood/run_all.sh                       # needs a display for 3 tasks
# Under Xvfb on a Wayland desktop, unset WAYLAND_DISPLAY (else winit opens a
# real window), and give wgpu a presentable adapter, e.g. lavapipe:
#   VK_ICD_FILENAMES=.../lvp_icd.x86_64.json
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
- `src/bin/*.rs` are the tasks. `t0` to `t5_live` are written against `gup_core`
  (GUP-465); `ref_export_png`, `t5_stream` and `t6_*` still use the old `gup`
  API until RFC-001 S12 and S14. When the new core replaces that API, rewrite
  the remaining binaries. The manifest stays put, so the bar the task has to
  clear does not move. RFC-001 step S10's exit criterion is "dogfood tasks 1-3
  pass".
- `src/bin/dogfood_check.rs` is the runner.

## The tasks

Gaps marked _(tracked)_ are encoded in `src/suite.rs` and checked on every run.
The others are recorded here, from the task sources, but have no pixel check.

### Control and reference

- **`t0_smoke`**: the simplest scatter (20 points) with a title and axes,
  exported to PNG through `gup_core::scatter`. Works: title, tick labels and 20
  separate points, each checked to within 1 px of its datum's pixel (the default
  dots are smaller than the old builder's stretched ellipses, so a plain
  coverage fraction no longer fits). The audit also tried the README-style
  `plot().scatter(x("x"), y("y"))` route; GUP-389 deleted that API.
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

- `gup_core::line(rows)` with Unix-second `f64` x values and `Time::new()`, an
  `Option<f64>` y (a `None` breaks the line), `.color(|r| r.ticker.as_str())`
  with an explicit palette, `.width(1.5)`, `.grid(Axes::Y)` and a title. No
  hand-computed domain, plot rect, legend or SVG workaround.
- Works: title, date ticks, legend at the right, grid, exact series colours.
- Dropped: the old task's `%Y-%m` tick format (the time scale chooses its own
  ticks).

### Task 2: `t2_bars`

A grouped and a stacked bar chart (4 regions x 4 quarters) with legend, exported
to PNG through `gup_core::bar` with `.group(..)` / `.stack(..)`.

- Works: grouped bars (16), stacked bars (4), palette colours, title and legend.
  The old workaround part is gone.
- Gap (untracked): there are no value labels, as before and unchecked.

### Task 3: `t3_scatter_png` and `t3_scatter_window`

A 200k-point scatter: colour = categorical segment, size = numeric weight, log x
axis.

- `t3_scatter_png`: `gup_core::scatter` with `Log::new()`, a palette, `size`,
  `opacity(0.35)`, `grid(Axes::Both)` and a title. All five segments, title,
  ticks and grid render. Categorical colours follow first-seen order, so the
  manifest labels colours, not segments. The SVG is a full-scene `SvgTarget`
  render and has all 200k `<circle>` marks (GUP-467).
- Dropped: the old `$Nk` x tick format.
- `t3_scatter_window`: the same chart with `.tooltip(..)` in `gup_core::show`.
  With `DOGFOOD_AUTO=1` it resolves headless, takes a row's `pick_anchor`, and
  sets `GUP_SCREENSHOT_*` so the window hovers there. The window check
  recognises a tooltip panel of any style (`Measure::Tooltip`) on the default
  tooltip. egui is not used.

### Task 4: `t4_linked`

Two linked scatter views (30k points) in egui through `ChartTexture`, which
renders any `gup_core::Chart`. egui pointer events go to the left chart's
`Chart::handle`; it uses `Drag::Brush` and `on_brush` writes the brushed rows to
shared state. The right chart is rebuilt from the rows whenever the selection
changes (grey for unselected, colours for selected). This **works**. The left
chart's own dimming of unbrushed rows is set to 0 (`unbrushed_opacity`), because
faded colours still read as colour and the check wants grey only there.

### Task 5: `t5_live` and `t5_stream`

Append points every 100 ms without rebuilding the chart.

- `t5_live`: a line chart (random walk) and a 100k-point scatter on `gup_core`.
  This **works**, but there is no append yet (RFC-001 S12's `appender`). Every
  tick rebuilds both charts from their rows, a full re-upload, as before. The
  scatter costs tens of milliseconds per tick to render.
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
