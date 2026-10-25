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
`gup = { path = ".." }`, which since the RFC-001 S14 flip (GUP-488) is a facade
over `gup-core`. As a result it:

- is **not** a member of the parent workspace, so `cargo build` at the repo root
  does not build it, and it cannot see `pub(crate)` items;
- consumes `gup` exactly as a downstream user would. Every import here is public
  API, and every helper in `src/lib.rs` is code a user had to write because Gup
  doesn't provide it.

## Running it

```sh
./dogfood/run_all.sh                       # headless: lavapipe + Xvfb 1920x1080
DOGFOOD_DISPLAY=1 ./dogfood/run_all.sh     # the real display (windowed tasks)
DOGFOOD_SKIP_WINDOWED=1 ./dogfood/run_all.sh
DOGFOOD_ONLY=t1_timeseries,t2_bars ./dogfood/run_all.sh
mask dogfood                               # same as the first line
```

The script builds every binary in release mode and runs
`target/release/dogfood_check` under `scripts/headless_env.sh --run` (a
forgotten ICD or `WAYLAND_DISPLAY` can no longer produce a false FAIL). Outputs,
CSV fixtures and per-task stdout/stderr logs go to `/tmp/gup-dogfood/`.
`DOGFOOD_TIMEOUT_SECS` sets the per-task timeout (default 300).

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
- The pixel measurements (background, coverage, ink, hue, exact colour, grey,
  horizontal runs, tooltip panels) live in the shared
  `gup_visual_regression::measure` module (`crates/gup-visual-regression`,
  GUP-397); the suite depends on it by path and reads PNGs with its `RgbaImage`.
- `src/bin/*.rs` are the tasks. `t0` to `t5_stream` are written against `gup`
  (GUP-465, GUP-480, GUP-485). The manifest stays put, so the bar the task has
  to clear does not move. RFC-001 step S10's exit criterion is "dogfood tasks
  1-3 pass".
- `src/bin/dogfood_check.rs` is the runner.

## The tasks

Gaps marked _(tracked)_ are encoded in `src/suite.rs` and checked on every run.
The others are recorded here, from the task sources, but have no pixel check.

### Control and reference

- **`t0_smoke`**: the simplest scatter (20 points) with a title and axes,
  exported to PNG through `gup::scatter`. Works: title, tick labels and 20
  separate points, each checked to within 1 px of its datum's pixel (the default
  dots are smaller than the old builder's stretched ellipses, so a plain
  coverage fraction no longer fits). The audit also tried the README-style
  `plot().scatter(x("x"), y("y"))` route; GUP-389 deleted that API.
- **`ref_export_png`**: a titled, gridded scatter of ten points (a plain
  `circles` layer on a `Plot`), exported at 800x600, as a 1600x1200 `@2x` image
  (`dpr` 2) and at 2400x1600, built as an external crate. Title, ticks, grid and
  steel-blue points with dark rims render as configured; no tracked failures.
  The old subtitle is dropped (`gup-core` has no subtitle).

### Task 1: `t1_timeseries`

Multi-line time series (three tickers, five years of daily prices with gaps)
from CSV, with a date axis, legend and title, exported to PNG.

- `gup::line(rows)` with Unix-second `f64` x values and `Time::new()`, an
  `Option<f64>` y (a `None` breaks the line), `.color(|r| r.ticker.clone())`
  with an explicit palette, `.width(1.5)`, `.grid(Axes::Y)` and a title. No
  hand-computed domain, plot rect, legend or SVG workaround.
- Works: title, date ticks, legend at the right, grid, exact series colours.
- Dropped: the old task's `%Y-%m` tick format (the time scale chooses its own
  ticks).

### Task 2: `t2_bars`

A grouped and a stacked bar chart (4 regions x 4 quarters) with legend, exported
to PNG through `gup::bar` with `.group(..)` / `.stack(..)`.

- Works: grouped bars (16), stacked bars (4), palette colours, title and legend.
  The old workaround part is gone.
- Gap (untracked): there are no value labels, as before and unchecked.

### Task 3: `t3_scatter_png` and `t3_scatter_window`

A 200k-point scatter: colour = categorical segment, size = numeric weight, log x
axis.

- `t3_scatter_png`: `gup::scatter` with `Log::new()`, a palette, `size`,
  `opacity(0.35)`, `grid(Axes::Both)` and a title. All five segments, title,
  ticks and grid render. Categorical colours follow first-seen order, so the
  manifest labels colours, not segments. The SVG is a full-scene `SvgTarget`
  render and has all 200k `<circle>` marks (GUP-467).
- Dropped: the old `$Nk` x tick format.
- `t3_scatter_window`: the same chart with `.tooltip(..)` in `gup::show`. With
  `DOGFOOD_AUTO=1` it resolves headless, takes a row's `pick_anchor`, and sets
  `GUP_SCREENSHOT_*` so the window hovers there. The window check recognises a
  tooltip panel of any style (`Measure::Tooltip`) on the default tooltip. egui
  is not used.

### Task 4: `t4_linked`

Two linked scatter views (30k points) in egui through `gup_egui::GupWidget`,
which renders any `gup::Chart` on egui's device. egui pointer events reach the
left chart's `Chart::handle`; it uses `Drag::Brush` and `on_brush` writes the
brushed rows to shared state. The right chart is rebuilt from the rows whenever
the selection changes (grey for unselected, colours for selected). This
**works**. The left chart's own dimming of unbrushed rows is `Unbrushed::Grey`
(a theme setting): rows outside the brush are drawn in `#bfbfbf`, visible but
colourless, because a fade keeps the hue and the check wants grey only there.
Two checks guard it: grey only (at most 20 saturated pixels) and greyed, not
hidden (at least 500 grey pixels).

### Task 5: `t5_live` and `t5_stream`

Append points every 100 ms without rebuilding the chart.

- `t5_live`: a line chart (random walk) and a 100k-point scatter on `gup`. Each
  chart is built once and fed through `appender()`: every tick `extend`s the new
  samples and the next frame uploads only those rows (GUP-480). The window wakes
  when rows arrive.
- `t5_stream`: a scatter chart built empty; three batches of 50 rows go through
  its `appender()`, with a PNG after each. Three checks on `t5_stream_2.png`
  find each batch in its third of the plot.

### Task 6: `t6_wgsl`

Two custom `#[wgsl_function]` GPU transforms in a downstream crate: a soft-knee
radius (`Knee`, in `Px`) and a blue-to-orange-red colour ramp (`Ramp`,
`vec4<f32>`), applied to `Circle::RADIUS` and `Circle::FILL` of a 20x20 grid on
a plot with fixed -1..1 domains.

- The macro compiles in an external crate with no workarounds (`Params` must be
  padded; the macro reports the rule at compile time).
- Point radii grow left to right and the fill runs blue to orange-red, at the
  exact configured values. No tracked failures.

## Fixtures

`gen_data` writes `prices.csv`, `sales.csv` and `points.csv` (200k rows) to
`/tmp/gup-dogfood/` from a fixed-seed generator, so outputs are reproducible. It
has no Gup dependency.
