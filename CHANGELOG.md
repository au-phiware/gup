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

## [Unreleased]

## [0.1.0] - Unreleased

### Added

- Eight chart builders over your own structs: `gup::scatter`, `line`, `bar`,
  `barh`, `area`, `histogram`, `heatmap` and `boxplot`, each configured with
  closures.
- `Plot` and `Selection`, the layered API the builders are made of: several
  layers of marks on shared scales, with a second y axis.
- Marks: `Circle`, `Rect`, `Line` and `Area`, with typed channels, so a wrong
  value type or an unknown channel does not compile.
- Line curves, including steps and monotone curves, and stacked areas and bars.
- Text layers (`Plot::text`) for annotations, and bar value labels.
- `#[wgsl_function]` to write your own GPU encoding with an exact CPU mirror,
  and `#[derive(Mark)]` to define your own mark.
- A scale family: linear, pow, log, symlog, time, band, point and colour scales,
  with ticks, axes and legends computed to match the GPU.
- Legends, titles and subtitles, tick formats and a built-in `Theme::dark`.
- Render targets: a window (`gup::show`), PNG, SVG, a texture, and a WebGPU
  canvas in the browser.
- Interaction: picking, tooltips, wheel zoom, drag pan and brushing. Zooming and
  panning write no column data to the GPU.
- Streaming: `append` and `Appender` to add rows while a chart is showing, and
  `Window` to bound how many rows are kept.
- `gup-egui`, a widget that shows a chart in an egui panel, and `gup-bevy`, a
  plugin that shows a chart in a Bevy app.
- Seven tutorials, from a first chart to custom marks and a tour of the chart
  types, and runnable examples.

### Known limitations

- No CSV, serde or Arrow input: rows enter through closures.
- Nothing above 10M rows is measured, and there is no level of detail.
- No PDF export.
- No accessibility support yet: charts have no text alternative and no keyboard
  navigation.
- CI renders on Linux only; Windows is not yet tested.
- Chart types are those the builders list today.
