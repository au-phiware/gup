# Changelog

All notable changes to Gup are recorded here, in the
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format. This page says
what each release contains and what its version number promises.

## Versioning

- All seven crates share one version and are released together. Each depends on
  the others at exactly that version.
- Before 1.0, a release that breaks the public API raises the minor version (0.1
  to 0.2). A release that only adds or fixes raises the patch version (0.1.0 to
  0.1.1).
- The public API is every public item reachable from `gup`, including its
  re-exports of `gup-core`, except items marked `#[doc(hidden)]` and the
  `__private` module. Changes to `gup-core-macros`, `gup-wgsl` and `gup-text`
  that are invisible through `gup` follow the same numbers, because the crates
  move in lockstep.
- `gup::wgpu` is a re-export, so a wgpu major upgrade is a breaking change and
  raises the minor version.
- Raising `rust-version` (the minimum Rust version, 1.93 for 0.1) raises the
  minor version.
- Rendered output is not API. Pixel-level changes (antialiasing, layout tweaks)
  can come in a patch, and are listed under "Changed".
- Default feature changes and removals are breaking.

## [0.1.0] - Unreleased

### Added

- The [accessibility guide](docs/ACCESSIBILITY.md): what each feature does, what
  you must do and what is not supported.
- `ChartExt::to_svg` and `ChartExt::save_svg` (feature `svg`): a chart renders
  itself as SVG in one call, like `save_png`.
- `TextLayer::fallbacks`: with `hide_overlaps(true)`, a label that collides
  tries a ranked list of `(Anchor, dx, dy)` positions before it is hidden.
- `TextLayer::avoid_points`: with `hide_overlaps(true)`, labels keep off the
  points (a constant px radius) and are never nearer another point than their
  own.
- Eight chart builders over your own structs: `gup::scatter`, `line`, `bar`,
  `barh`, `area`, `histogram`, `heatmap` and `boxplot`, each configured with
  closures.
- The window app (`gup::show`, `GupApp`) and the egui widget (`GupWidget`)
  forward keyboard input to the chart: arrows, Home, End, Enter, Space, Escape,
  `+`/`=`, `-` and `0`. In egui the widget takes keys only while it has focus
  (click it or Tab into it); Escape in a window closes it only when the chart
  had no focused row to clear.
- `Theme::high_contrast()`, for low-vision readers: black text and axes, a grid
  at 3:1, larger text, and the `HIGH_CONTRAST` palette whose eight colours are
  each at least 3:1 on white and clearly distinct under normal, deutan and
  protan vision.
- The chart types (`LineChart`, `ScatterChart`, and so on) are in
  `gup::prelude`, so one `use gup::prelude::*;` names the type a function
  returns; `Horizontal` and `Vertical` are at the crate root.
- `Prepared::encode` records translucent lines' offscreen pass before a host's
  own render pass, so a host that draws with `Renderer::prepare` and
  `Prepared::draw` gets lines whose joins and crossings are as opaque as their
  arms, as in Gup's own targets. Without the call they still draw, blending
  twice at overlaps. `zoom_bench --translucent` times it.
- `Renderer::prepare` returns an error for an `*Srgb` target format, which would
  blend translucent marks in linear light and draw them lighter than Gup's own
  targets; `Prepared::draw` documents drawing through a non-sRGB view.
- Every builder has `.text(rows, |t| ..)` to add labels at data positions and a
  `pick_row` that takes a tuple, so `chart.pick_row((x, y))` compiles without
  `plot_mut()`.
- `Plot` and `Selection`, the layered API the builders are made of: several
  layers of marks on shared scales, with a second y axis.
- Marks: `Circle`, `Rect`, `Line` and `Area`, with typed channels, so a wrong
  value type or an unknown channel does not compile.
- Line curves, including steps and monotone curves, and stacked areas and bars.
- The x axis of a `gup::line` or `gup::area` chart ends at the first and last
  rows, with no empty strip at either edge; an explicit `.x_scale(..)` domain
  still wins.
- Legends match what is drawn: a vertical stack (bars, areas, histograms) lists
  its keys top-down, and swatches of translucent marks show the blended colour.
- Text layers (`Plot::text`) for annotations, and bar value labels.
- `#[wgsl_function]` to write your own GPU encoding with an exact CPU mirror,
  and `#[derive(Mark)]` to define your own mark.
- A scale family: linear, pow, log, symlog, time, band, point and colour scales,
  with ticks, axes and legends computed to match the GPU.
- Legends, titles and subtitles, tick formats and a built-in `Theme::dark`.
- Keyboard focus over a chart's rows: send `Input::key(Key::Right)` (and `Left`,
  `Up`, `Down`, `Home`, `End`, `Enter`, `Escape`) to a plot, and it moves a
  focus ring and tooltip from row to row; `Plot::focused` says which.
  `Selection::focusable(false)` keeps a helper layer out of the walk.
- Keyboard zoom, pan and reset: `Key::ZoomIn` and `Key::ZoomOut` zoom about the
  centre of the plot, Shift with an arrow pans a tenth of the plot, and
  `Key::ResetView` restores the view. Keys held with Ctrl, Alt or Meta are left
  to the host.
- Row announcements for screen readers: `Plot::focus_text` (and
  `Chart::focus_text`) says the focused row's values with their axis names and
  its place, such as `Mass: 0.9; Span: 0.95 (2 of 3)`, in the axes' own formats
  (a time axis says an ISO date). `Plot::row_fields` and `Plot::row_text` give
  any row's values as `(name, value)` pairs or text.
- Keyboard and screen readers in the browser: `web::CanvasApp` makes the canvas
  focusable (`tabindex="0"` unless the page set one), forwards the chart keys
  (arrows, Home, End, Enter, Space, Escape, `+`, `-`, `0`) and speaks
  `Chart::focus_text` through a visually hidden `aria-live` element after the
  canvas.
- Render targets: a window (`gup::show`), PNG, SVG, a texture, and a WebGPU
  canvas in the browser.
- Interaction: picking, tooltips, wheel zoom, drag pan and brushing. Zooming and
  panning write no column data to the GPU.
- Streaming: `append` and `Appender` to add rows while a chart is showing, and
  `Window` to bound how many rows are kept.
- Columnar layers: `Plot::add_columns` draws `Columns` of `f64` or `f32`
  directly, with `x_column`, `y_column` and `encode_column`, without a row type
  or a closure per row. Gup keeps no copy of the columns: a 10M-point scatter
  loads in about 40% of the time of the same rows, using the columns plus a few
  MiB. A columnar layer above `Retain::AUTO_MAX_ROWS` still answers `pick_row`,
  re-encodes a channel, draws on another context and writes SVG from its
  columns.
- Key columns: `KeyColumn` (dictionary codes plus labels, `Column::Keys`)
  colours or positions a columnar layer by category on a `Categorical`, Band or
  Point scale, with no key closure and no string hash per row. At 10M rows with
  50 labels, the first resolve takes about 30-45% of the time of the same rows
  with a key closure.
- `gup-egui`, a widget that shows a chart in an egui panel, and `gup-bevy`, a
  plugin that shows a chart in a Bevy app.
- A text alternative for every chart: `Plot::describe` returns a title and a
  description in words, SVG files carry them as `title`, `desc` and
  `role="img"`, and the browser canvas gets an `aria-label`.
- `Plot::data_table` and `Chart::data_table` (`DataTable`): the chart's rows as
  a captioned table of the row fields, and in the browser `CanvasApp` carries it
  as a visually hidden `<table data-gup-table>` (up to 1,000 rows, rebuilt when
  the description changes) for screen-reader table navigation.
- The `gup` crate as the one dependency to add, with features `window` and `svg`
  (both on by default) and `web` (the browser canvas, wasm32 only). Every
  supported combination of features builds and tests.
- A gallery of ten charts, each a runnable example with a checked golden image,
  and a page that shows them.
- Seven tutorials, from a first chart to custom marks and a tour of the chart
  types, and runnable examples.
- A feature tour, `docs/tour/`: short pages, each with a snippet, the image it
  draws and a link to the tutorial; a test draws every snippet and checks that
  each page shows that code.
- CI that renders and compares goldens on Linux (Vulkan), Windows (DX12, WARP)
  and macOS (Metal), a feature-combination matrix, and a publish dry run with a
  docs.rs build of all seven crates.

### Changed

- The default categorical palette order is `OKABE_ITO_DISTINCT` (blue,
  vermillion, reddish purple, bluish green, sky blue, orange, yellow, black),
  with `OKABE_ITO_DISTINCT_DARK` for `Theme::dark` (sky blue first, as blue is
  dim on dark): the first four series are four different hues, and series 1 is
  the marks' default blue. `OKABE_ITO` and `OKABE_ITO_DARK` keep the published
  order. A single-series builder chart is now blue rather than orange.

### Known limitations

- No CSV, serde or Arrow input: rows enter through closures.
- Nothing above 10M rows is measured, and there is no level of detail.
- No PDF export.
- Accessibility: no screen reader has been tested by hand, nothing outside the
  browser speaks `focus_text`, and keyboard focus skips a layer that dropped its
  CPU data above 10M visible rows. See `docs/ACCESSIBILITY.md`.
- No pie, violin, density or choropleth charts, and no facets.
