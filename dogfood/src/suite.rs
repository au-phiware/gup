// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The dogfood suite manifest: what each task is for and what its output
//! must look like.
//!
//! This file contains no gup API calls. Each task binary under `src/bin/`
//! is the "how" (today written against the old `gup` API); this manifest is
//! the "what": the task's intent, the files it must produce and the checks
//! those files must pass. Re-pointing a task at a new API means rewriting
//! its binary; the manifest, and therefore the bar it has to clear, stays.
//!
//! Known gaps are tracked, not hidden. A check carrying a [`Gap`] is
//! *expected to fail*; if it starts passing the runner reports `XPASS` and
//! fails the build so the gap entry gets removed rather than going stale.
//! Never fix a gap by loosening its check: fix gup, then delete the `gap`.
//!
//! Regions are fractions of the output image, chosen from today's layout
//! (margins, title band, x-axis label band). If a layout change moves the
//! plot rect, re-derive the regions from a fresh render.

use crate::pixels::{self, Region};

/// Directory every task writes its fixtures and outputs to.
pub const OUT_DIR: &str = "/tmp/gup-dogfood";

/// A known, tracked gap: the reason a check (or exit status) is expected to
/// fail today, naming the story or track that should fix it.
#[derive(Debug, Clone, Copy)]
pub struct Gap(pub &'static str);

/// How a task binary is expected to exit.
#[derive(Debug, Clone, Copy)]
pub enum Exit {
    /// Exits 0.
    Success,
    /// Tracked expected failure: exits non-zero and stderr contains the
    /// given text. Exiting 0 is reported as `XPASS`.
    Fails {
        stderr_contains: &'static str,
        gap: Gap,
    },
}

/// Whether a task needs a display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Renders off-screen (PNG/SVG export).
    Headless,
    /// Opens a window; run with `DOGFOOD_AUTO=1` (scripted input, then a
    /// screenshot, then exit). Needs a display (Xvfb in CI).
    Windowed,
}

/// A measurement on one output file.
#[derive(Debug, Clone, Copy)]
pub enum Measure {
    /// At least this fraction of the whole image differs from the
    /// background colour.
    NotBlank { min_fraction: f32 },
    /// The fraction of non-background pixels in `region` lies in
    /// `min..=max`.
    Coverage { region: Region, min: f32, max: f32 },
    /// At least `min_px` dark "ink" pixels inside `region` — text (titles,
    /// legends, tick labels) in an otherwise empty margin.
    Ink { region: Region, min_px: usize },
    /// At least `min_px` pixels in `region` share the hue of `rgb` (0..1
    /// sRGB). Robust to alpha blending and gamma errors: answers "is this
    /// colour family visible at all".
    Hue {
        region: Region,
        rgb: [f32; 3],
        min_px: usize,
    },
    /// At least `min_px` pixels in `region` are within `tol` (per 8-bit
    /// channel) of `rgb` — "the configured colour is what renders".
    Colour {
        region: Region,
        rgb: [f32; 3],
        tol: u8,
        min_px: usize,
    },
    /// At least `min_px` light-to-mid grey pixels in `region` (grid lines,
    /// de-emphasised marks).
    Grey { region: Region, min_px: usize },
    /// At most `max_px` saturated pixels (any hue) in `region`: the region
    /// should be neutral — e.g. no data marks outside the plot rect.
    NoColour { region: Region, max_px: usize },
    /// The row at fraction `y`, between `x0` and `x1`, crosses between
    /// `min` and `max` separate runs of non-background pixels (bars).
    Runs {
        y: f32,
        x0: f32,
        x1: f32,
        min: usize,
        max: usize,
    },
    /// An SVG file contains at least `min` `<tag` elements.
    SvgElements { tag: &'static str, min: usize },
}

/// One check on one output file.
#[derive(Debug, Clone, Copy)]
pub struct Check {
    /// Output file name inside [`OUT_DIR`].
    pub file: &'static str,
    /// What is being asserted, in user terms.
    pub what: &'static str,
    pub measure: Measure,
    /// `Some` when this check is a tracked known gap (expected to fail).
    pub gap: Option<Gap>,
}

/// A dogfood task: one binary run, plus checks on what it produced.
#[derive(Debug, Clone)]
pub struct Task {
    /// Short identifier shown in reports.
    pub name: &'static str,
    /// Binary under `src/bin/`.
    pub bin: &'static str,
    /// Extra environment variables.
    pub env: &'static [(&'static str, &'static str)],
    pub kind: Kind,
    /// What a real user is trying to achieve.
    pub intent: &'static str,
    pub exit: Exit,
    pub checks: Vec<Check>,
}

/// Result of evaluating one measure.
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    pub passed: bool,
    pub detail: String,
}

impl Measure {
    /// Evaluate against the file at `path`.
    pub fn evaluate_file(&self, path: &std::path::Path) -> Measured {
        let fail = |e: String| Measured {
            passed: false,
            detail: format!("cannot read {}: {e}", path.display()),
        };
        if let Measure::SvgElements { tag, min } = *self {
            return match std::fs::read_to_string(path) {
                Ok(svg) => {
                    let n = svg.matches(&format!("<{tag}")).count();
                    Measured {
                        passed: n >= min,
                        detail: format!("{n} <{tag}> elements (need >= {min})"),
                    }
                }
                Err(e) => fail(e.to_string()),
            };
        }
        match image::open(path) {
            Ok(img) => self.evaluate(&img.to_rgba8()),
            Err(e) => fail(e.to_string()),
        }
    }

    /// Evaluate against a decoded image.
    pub fn evaluate(&self, img: &image::RgbaImage) -> Measured {
        let at_least = |n: usize, min: usize, unit: &str| Measured {
            passed: n >= min,
            detail: format!("{n} {unit} (need >= {min})"),
        };
        let pct = |f: f32| format!("{:.2}%", f * 100.0);
        match *self {
            Measure::NotBlank { min_fraction } => {
                let f = pixels::non_background_fraction(img, Region::ALL, 8);
                Measured {
                    passed: f >= min_fraction,
                    detail: format!("{} non-background (need >= {})", pct(f), pct(min_fraction)),
                }
            }
            Measure::Coverage { region, min, max } => {
                let f = pixels::non_background_fraction(img, region, 8);
                Measured {
                    passed: (min..=max).contains(&f),
                    detail: format!("{} covered (need {}..={})", pct(f), pct(min), pct(max)),
                }
            }
            Measure::Ink { region, min_px } => {
                at_least(pixels::count_ink(img, region, 90.0), min_px, "ink px")
            }
            Measure::Hue {
                region,
                rgb,
                min_px,
            } => {
                let n = pixels::count_hue(img, region, pixels::hue_of(rgb), 15.0, 0.15);
                at_least(n, min_px, "px of that hue")
            }
            Measure::Colour {
                region,
                rgb,
                tol,
                min_px,
            } => {
                let target = rgb.map(|c| (c * 255.0).round() as u8);
                at_least(
                    pixels::count_near(img, region, target, tol),
                    min_px,
                    &format!("px within {tol} of {target:?}"),
                )
            }
            Measure::Grey { region, min_px } => at_least(
                pixels::count_grey(img, region, 0.08, 120.0),
                min_px,
                "grey px",
            ),
            Measure::NoColour { region, max_px } => {
                let n = pixels::count_saturated(img, region, 0.25);
                Measured {
                    passed: n <= max_px,
                    detail: format!("{n} saturated px (allow <= {max_px})"),
                }
            }
            Measure::Runs {
                y,
                x0,
                x1,
                min,
                max,
            } => {
                let n = pixels::count_runs(img, y, x0, x1, 3);
                Measured {
                    passed: (min..=max).contains(&n),
                    detail: format!("{n} runs (need {min}..={max})"),
                }
            }
            Measure::SvgElements { .. } => Measured {
                passed: false,
                detail: "SvgElements must be evaluated on a file".into(),
            },
        }
    }
}

// --------------------------------------------------------------------------
// Known gaps (each one observed in the task outputs on 2026-10-04)
// --------------------------------------------------------------------------

const NO_TEXT: Gap =
    Gap("PNG/texture output renders no text: no title, tick labels or legend (RFC-001 S2/S3)");
const DOUBLE_GAMMA: Gap = Gap(
    "colours are gamma-encoded twice, e.g. #1f77b4 renders ~#8ebddb (T4a; RFC-001 S3 colour policy)",
);
const NO_GRID: Gap = Gap(
    "grid is configured but not drawn in PNG/texture output; SVG has it (RFC-001 S3/S7 guides)",
);
const SVG_NO_MARKS: Gap =
    Gap("SVG export has axes, grid and text but no data marks (RFC-001 S3 SvgTarget)");
const NO_LEGEND: Gap = Gap("no legend API (T5; RFC-001 S7 guides)");
const NO_GROUPING: Gap =
    Gap("bar group_by does not change geometry: one overlapping bar per x band (T5)");
const NO_STACKING: Gap = Gap("bar stack_by does not stack; bars overlap (T5)");
const CATEGORICAL_GREY: Gap =
    Gap("bar .color() with a String value renders grey instead of using a palette (T5)");
const STREAM_NEVER_READY: Gap = Gap(
    "a DataStream-fed Selection never becomes render-ready: prepare_render_bound fails with \
     \"No attribute bindings set\" (RFC-001 S12 append replaces this route)",
);
const WGSL_DUPLICATE_UNIFORMS: Gap = Gap(
    "#[wgsl_function] output embeds its uniforms struct and Selection's shader assembly emits it \
     again (RFC-001 S5 ShaderFn v2 / module composition)",
);
const SELECTION_CLIP_SPACE: Gap = Gap(
    "a raw Selection draws in whole-canvas clip space while ComposedChart axes use the plot rect, \
     so data mapped with the axis scales lands outside the axes (RFC-001 S7 Layout)",
);

// --------------------------------------------------------------------------
// Palette and regions shared by several tasks
// --------------------------------------------------------------------------

const BLUE: [f32; 3] = [0.122, 0.467, 0.706];
const ORANGE: [f32; 3] = [1.000, 0.498, 0.055];
const GREEN: [f32; 3] = [0.173, 0.627, 0.173];
const RED: [f32; 3] = [0.839, 0.153, 0.157];
const PURPLE: [f32; 3] = [0.580, 0.404, 0.741];

/// Centred band above the plot where a chart title goes.
const fn title_band(bottom: f32) -> Region {
    Region::new(0.3, 0.0, 0.7, bottom)
}

fn check(file: &'static str, what: &'static str, measure: Measure) -> Check {
    Check {
        file,
        what,
        measure,
        gap: None,
    }
}

fn gap(file: &'static str, what: &'static str, measure: Measure, gap: Gap) -> Check {
    Check {
        file,
        what,
        measure,
        gap: Some(gap),
    }
}

// --------------------------------------------------------------------------
// The manifest
// --------------------------------------------------------------------------

/// Fixture generator; must succeed before any task runs.
pub fn setup() -> Task {
    Task {
        name: "gen_data",
        bin: "gen_data",
        env: &[],
        kind: Kind::Headless,
        intent: "write the CSV fixtures (prices, sales, 200k points)",
        exit: Exit::Success,
        checks: vec![],
    }
}

/// Every task, in run order.
pub fn tasks() -> Vec<Task> {
    vec![
        t0_smoke(),
        ref_export_png(),
        t1(),
        t2(),
        t3_png(),
        t5_stream(),
        t6_tutorial(),
        t6_workaround(),
        t3_window(),
        t4(),
        t5_live(),
    ]
}

fn t0_smoke() -> Task {
    let f = "t0_smoke_builder.png";
    Task {
        name: "t0_smoke",
        bin: "t0_smoke",
        env: &[],
        kind: Kind::Headless,
        intent: "control: the simplest scatter with a title and axes, exported to PNG",
        exit: Exit::Success,
        checks: vec![
            check(
                f,
                "chart is not blank",
                Measure::NotBlank {
                    min_fraction: 0.003,
                },
            ),
            check(
                f,
                "points drawn inside the plot",
                Measure::Coverage {
                    region: Region::new(0.17, 0.08, 0.94, 0.8),
                    min: 0.01,
                    max: 0.5,
                },
            ),
            gap(
                f,
                "title text present",
                Measure::Ink {
                    region: title_band(0.06),
                    min_px: 30,
                },
                NO_TEXT,
            ),
            gap(
                f,
                "x tick labels present",
                Measure::Ink {
                    region: Region::new(0.17, 0.85, 0.97, 0.97),
                    min_px: 30,
                },
                NO_TEXT,
            ),
        ],
    }
}

fn ref_export_png() -> Task {
    let f = "ref_export_png.png";
    Task {
        name: "ref_export_png",
        bin: "ref_export_png",
        env: &[],
        kind: Kind::Headless,
        intent: "reference: gup's own PNG export example (Selection + ComposedChart, title, grid), built externally",
        exit: Exit::Success,
        checks: vec![
            check(
                f,
                "chart is not blank",
                Measure::NotBlank {
                    min_fraction: 0.003,
                },
            ),
            check(
                f,
                "steel-blue points visible",
                Measure::Hue {
                    region: Region::ALL,
                    rgb: [0.22, 0.46, 0.82],
                    min_px: 400,
                },
            ),
            check(
                "ref_export_png@2x.png",
                "2x export is not blank",
                Measure::NotBlank {
                    min_fraction: 0.002,
                },
            ),
            check(
                "ref_export_large.png",
                "large export is not blank",
                Measure::NotBlank {
                    min_fraction: 0.002,
                },
            ),
            gap(
                f,
                "steel blue renders as configured",
                Measure::Colour {
                    region: Region::ALL,
                    rgb: [0.22, 0.46, 0.82],
                    tol: 12,
                    min_px: 500,
                },
                DOUBLE_GAMMA,
            ),
            gap(
                f,
                "title text present",
                Measure::Ink {
                    region: title_band(0.08),
                    min_px: 30,
                },
                NO_TEXT,
            ),
            gap(
                f,
                "grid lines drawn in the empty plot area",
                Measure::Grey {
                    region: Region::new(0.55, 0.55, 0.9, 0.78),
                    min_px: 300,
                },
                NO_GRID,
            ),
            gap(
                f,
                "no points left of the y axis",
                Measure::NoColour {
                    region: Region::new(0.0, 0.05, 0.15, 0.95),
                    max_px: 0,
                },
                SELECTION_CLIP_SPACE,
            ),
        ],
    }
}

fn t1() -> Task {
    let (png, svg) = ("t1.png", "t1_svg.png");
    let plot = Region::new(0.13, 0.08, 0.96, 0.78);
    Task {
        name: "t1_timeseries",
        bin: "t1_timeseries",
        env: &[],
        kind: Kind::Headless,
        intent: "task 1: multi-line time series from CSV with date axis, legend and title, exported to PNG",
        exit: Exit::Success,
        checks: vec![
            check(
                png,
                "chart is not blank",
                Measure::NotBlank { min_fraction: 0.01 },
            ),
            check(
                png,
                "AAPL series (blue) drawn",
                Measure::Hue {
                    region: plot,
                    rgb: BLUE,
                    min_px: 300,
                },
            ),
            check(
                png,
                "MSFT series (orange) drawn",
                Measure::Hue {
                    region: plot,
                    rgb: ORANGE,
                    min_px: 300,
                },
            ),
            check(
                png,
                "GOOG series (green) drawn",
                Measure::Hue {
                    region: plot,
                    rgb: GREEN,
                    min_px: 300,
                },
            ),
            gap(
                png,
                "blue series renders as configured",
                Measure::Colour {
                    region: plot,
                    rgb: [0.12, 0.47, 0.71],
                    tol: 24,
                    min_px: 50,
                },
                DOUBLE_GAMMA,
            ),
            gap(
                png,
                "title text present",
                Measure::Ink {
                    region: title_band(0.07),
                    min_px: 30,
                },
                NO_TEXT,
            ),
            gap(
                png,
                "date tick labels present",
                Measure::Ink {
                    region: Region::new(0.15, 0.81, 0.97, 0.95),
                    min_px: 30,
                },
                NO_TEXT,
            ),
            gap(
                png,
                "legend present",
                Measure::Ink {
                    region: Region::new(0.13, 0.08, 0.19, 0.16),
                    min_px: 30,
                },
                NO_LEGEND,
            ),
            gap(
                png,
                "horizontal grid drawn",
                Measure::Grey {
                    region: Region::new(0.5, 0.1, 0.9, 0.4),
                    min_px: 300,
                },
                NO_GRID,
            ),
            // The workaround: gup's SVG export (axes + text) plus hand-built
            // line paths and legend, rasterised with ImageMagick.
            check(
                svg,
                "workaround SVG: title text present",
                Measure::Ink {
                    region: title_band(0.07),
                    min_px: 30,
                },
            ),
            check(
                svg,
                "workaround SVG: date tick labels present",
                Measure::Ink {
                    region: Region::new(0.15, 0.81, 0.97, 0.95),
                    min_px: 30,
                },
            ),
            check(
                svg,
                "workaround SVG: three series drawn",
                Measure::Hue {
                    region: plot,
                    rgb: GREEN,
                    min_px: 300,
                },
            ),
        ],
    }
}

fn t2() -> Task {
    let (grouped, stacked) = ("t2_grouped.png", "t2_stacked.png");
    let (grouped_wa, stacked_wa) = ("t2_grouped_wa.png", "t2_stacked_wa.png");
    let plot = Region::new(0.165, 0.08, 0.95, 0.78);
    // The first x band (Q1); a stacked bar shows every region's colour here.
    let q1 = Region::new(0.165, 0.08, 0.3, 0.78);
    let bars = |min, max| Measure::Runs {
        y: 0.77,
        x0: 0.165,
        x1: 0.95,
        min,
        max,
    };
    Task {
        name: "t2_bars",
        bin: "t2_bars",
        env: &[],
        kind: Kind::Headless,
        intent: "task 2: grouped and stacked bar chart (region x quarter) with legend and value labels, PNG",
        exit: Exit::Success,
        checks: vec![
            // Part A: the documented API.
            check(
                grouped,
                "grouped chart is not blank",
                Measure::NotBlank { min_fraction: 0.05 },
            ),
            gap(
                grouped,
                "grouped: 16 bars (4 quarters x 4 regions)",
                bars(16, 16),
                NO_GROUPING,
            ),
            gap(
                grouped,
                "grouped: North bars in palette blue",
                Measure::Hue {
                    region: plot,
                    rgb: BLUE,
                    min_px: 2000,
                },
                CATEGORICAL_GREY,
            ),
            gap(
                grouped,
                "grouped: West bars in palette red",
                Measure::Hue {
                    region: plot,
                    rgb: RED,
                    min_px: 2000,
                },
                CATEGORICAL_GREY,
            ),
            gap(
                grouped,
                "title text present",
                Measure::Ink {
                    region: title_band(0.07),
                    min_px: 30,
                },
                NO_TEXT,
            ),
            gap(
                grouped,
                "legend present above the plot",
                Measure::Ink {
                    region: Region::new(0.16, 0.0, 0.6, 0.08),
                    min_px: 30,
                },
                NO_LEGEND,
            ),
            check(
                stacked,
                "stacked chart is not blank",
                Measure::NotBlank { min_fraction: 0.05 },
            ),
            gap(
                stacked,
                "stacked: Q1 shows all four region colours",
                Measure::Hue {
                    region: q1,
                    rgb: GREEN,
                    min_px: 1000,
                },
                NO_STACKING,
            ),
            // Part B: the workaround (one band per quarter/region, explicit
            // colours, overlapping cumulative bars).
            check(grouped_wa, "workaround: 16 grouped bars", bars(16, 16)),
            check(
                grouped_wa,
                "workaround: four region colours",
                Measure::Hue {
                    region: plot,
                    rgb: RED,
                    min_px: 2000,
                },
            ),
            gap(
                grouped_wa,
                "workaround: North bars render palette blue exactly",
                Measure::Colour {
                    region: plot,
                    rgb: BLUE,
                    tol: 8,
                    min_px: 2000,
                },
                DOUBLE_GAMMA,
            ),
            check(stacked_wa, "workaround: 4 stacks", bars(4, 4)),
            check(
                stacked_wa,
                "workaround: Q1 stack shows green layer",
                Measure::Hue {
                    region: q1,
                    rgb: GREEN,
                    min_px: 1000,
                },
            ),
            check(
                stacked_wa,
                "workaround: Q1 stack shows red layer",
                Measure::Hue {
                    region: q1,
                    rgb: RED,
                    min_px: 1000,
                },
            ),
        ],
    }
}

fn t3_png() -> Task {
    let (png, svg) = ("t3_scatter.png", "t3_scatter.svg");
    let plot = Region::new(0.13, 0.06, 0.96, 0.84);
    Task {
        name: "t3_scatter_png",
        bin: "t3_scatter_png",
        env: &[],
        kind: Kind::Headless,
        intent: "task 3a: 200k-point scatter, colour = segment, size = weight, log x; PNG and SVG",
        exit: Exit::Success,
        checks: vec![
            check(
                png,
                "chart is not blank",
                Measure::NotBlank { min_fraction: 0.1 },
            ),
            check(
                png,
                "retail (blue) points",
                Measure::Hue {
                    region: plot,
                    rgb: BLUE,
                    min_px: 5000,
                },
            ),
            check(
                png,
                "wholesale (orange) points",
                Measure::Hue {
                    region: plot,
                    rgb: ORANGE,
                    min_px: 5000,
                },
            ),
            check(
                png,
                "online (green) points",
                Measure::Hue {
                    region: plot,
                    rgb: GREEN,
                    min_px: 1000,
                },
            ),
            check(
                png,
                "partner (red) points",
                Measure::Hue {
                    region: plot,
                    rgb: RED,
                    min_px: 2000,
                },
            ),
            check(
                png,
                "other (purple) points",
                Measure::Hue {
                    region: plot,
                    rgb: PURPLE,
                    min_px: 5000,
                },
            ),
            gap(
                png,
                "title text present",
                Measure::Ink {
                    region: title_band(0.05),
                    min_px: 30,
                },
                NO_TEXT,
            ),
            gap(
                png,
                "x tick labels present",
                Measure::Ink {
                    region: Region::new(0.15, 0.86, 0.97, 0.97),
                    min_px: 30,
                },
                NO_TEXT,
            ),
            gap(
                png,
                "grid drawn",
                Measure::Grey {
                    region: Region::new(0.2, 0.08, 0.5, 0.28),
                    min_px: 300,
                },
                NO_GRID,
            ),
            gap(
                svg,
                "SVG contains the data points",
                Measure::SvgElements {
                    tag: "circle",
                    min: 1000,
                },
                SVG_NO_MARKS,
            ),
        ],
    }
}

fn t5_stream() -> Task {
    Task {
        name: "t5_stream",
        bin: "t5_stream",
        env: &[],
        kind: Kind::Headless,
        intent: "task 5 (alt): Tutorial 5's DataStream -> Selection::stream route, PNG after each batch",
        exit: Exit::Success,
        checks: vec![
            gap(
                "t5_stream_0.png",
                "first batch visible",
                Measure::NotBlank {
                    min_fraction: 0.001,
                },
                STREAM_NEVER_READY,
            ),
            gap(
                "t5_stream_2.png",
                "all three batches visible",
                Measure::NotBlank {
                    min_fraction: 0.003,
                },
                STREAM_NEVER_READY,
            ),
        ],
    }
}

fn t6_tutorial() -> Task {
    Task {
        name: "t6_wgsl_tutorial",
        bin: "t6_wgsl",
        env: &[("RAW_MACRO", "1")],
        kind: Kind::Headless,
        intent: "task 6: a custom #[wgsl_function] GPU transform in a downstream crate, exactly as Tutorial 3 shows",
        exit: Exit::Fails {
            stderr_contains: "redefinition of `KneeRadiusUniforms`",
            gap: WGSL_DUPLICATE_UNIFORMS,
        },
        checks: vec![],
    }
}

fn t6_workaround() -> Task {
    let f = "t6_wgsl.png";
    Task {
        name: "t6_wgsl_workaround",
        bin: "t6_wgsl",
        env: &[],
        kind: Kind::Headless,
        intent: "task 6 workaround: the same transform, wrapped to strip the duplicate uniforms struct",
        exit: Exit::Success,
        checks: vec![
            check(
                f,
                "grid of points is not blank",
                Measure::NotBlank { min_fraction: 0.1 },
            ),
            check(
                f,
                "v=0 column: small radius",
                Measure::Coverage {
                    region: Region::new(0.0, 0.0, 0.08, 1.0),
                    min: 0.0,
                    max: 0.08,
                },
            ),
            check(
                f,
                "v=1 column: large radius",
                Measure::Coverage {
                    region: Region::new(0.92, 0.0, 1.0, 1.0),
                    min: 0.25,
                    max: 1.0,
                },
            ),
            // fill = [v, 0.3, 1 - v]: the v=1 column is red-orange.
            check(
                f,
                "v=1 column: orange-family fill",
                Measure::Hue {
                    region: Region::new(0.92, 0.0, 1.0, 1.0),
                    rgb: ORANGE,
                    min_px: 1000,
                },
            ),
            gap(
                f,
                "v=1 column: fill renders as configured",
                Measure::Colour {
                    region: Region::new(0.92, 0.0, 1.0, 1.0),
                    rgb: [1.0, 0.3, 0.0],
                    tol: 16,
                    min_px: 500,
                },
                DOUBLE_GAMMA,
            ),
        ],
    }
}

fn t3_window() -> Task {
    let f = "t3_window.png";
    // The chart texture sits under a 25 px egui title bar.
    let plot = Region::new(0.13, 0.09, 0.96, 0.83);
    Task {
        name: "t3_scatter_window",
        bin: "t3_scatter_window",
        env: &[],
        kind: Kind::Windowed,
        intent: "task 3b: the 200k scatter in a window with a hover tooltip showing the datum",
        exit: Exit::Success,
        checks: vec![
            check(
                f,
                "retail (blue) points",
                Measure::Hue {
                    region: plot,
                    rgb: BLUE,
                    min_px: 5000,
                },
            ),
            check(
                f,
                "other (purple) points",
                Measure::Hue {
                    region: plot,
                    rgb: PURPLE,
                    min_px: 5000,
                },
            ),
            check(
                f,
                "hover tooltip shown (hand-built hit test)",
                Measure::Ink {
                    region: Region::new(0.53, 0.54, 0.66, 0.67),
                    min_px: 6000,
                },
            ),
            gap(
                f,
                "x tick labels present",
                Measure::Ink {
                    region: Region::new(0.15, 0.85, 0.97, 0.93),
                    min_px: 30,
                },
                NO_TEXT,
            ),
        ],
    }
}

fn t4() -> Task {
    let f = "t4_linked.png";
    let right_view = Region::new(0.6, 0.05, 0.96, 0.75);
    Task {
        name: "t4_linked",
        bin: "t4_linked",
        env: &[],
        kind: Kind::Windowed,
        intent: "task 4: two linked scatter views in egui; brushing the left highlights the same customers on the right",
        exit: Exit::Success,
        checks: vec![
            check(
                f,
                "right view: brushed customers highlighted in colour",
                Measure::Hue {
                    region: right_view,
                    rgb: BLUE,
                    min_px: 500,
                },
            ),
            check(
                f,
                "right view: unbrushed customers de-emphasised grey",
                Measure::Grey {
                    region: right_view,
                    min_px: 2000,
                },
            ),
            check(
                f,
                "left view outside the brush: grey only",
                Measure::NoColour {
                    region: Region::new(0.125, 0.3, 0.215, 0.65),
                    max_px: 20,
                },
            ),
            check(
                f,
                "left view inside the brush: colour",
                Measure::Hue {
                    region: Region::new(0.23, 0.25, 0.36, 0.6),
                    rgb: PURPLE,
                    min_px: 500,
                },
            ),
        ],
    }
}

fn t5_live() -> Task {
    let f = "t5_live.png";
    Task {
        name: "t5_live",
        bin: "t5_live",
        env: &[],
        kind: Kind::Windowed,
        intent: "task 5: append points every 100 ms to a line chart and a 100k scatter without rebuilding",
        exit: Exit::Success,
        checks: vec![
            check(
                f,
                "initial line points drawn",
                Measure::Hue {
                    region: Region::new(0.14, 0.05, 0.18, 0.48),
                    rgb: RED,
                    min_px: 20,
                },
            ),
            check(
                f,
                "appended line points drawn",
                Measure::Hue {
                    region: Region::new(0.2, 0.05, 0.6, 0.48),
                    rgb: RED,
                    min_px: 20,
                },
            ),
            check(
                f,
                "scatter drawn",
                Measure::Hue {
                    region: Region::new(0.16, 0.55, 0.94, 0.78),
                    rgb: BLUE,
                    min_px: 2000,
                },
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::Path;

    #[test]
    fn task_names_are_unique() {
        let mut seen = HashSet::new();
        for t in std::iter::once(setup()).chain(tasks()) {
            assert!(seen.insert(t.name), "duplicate task name {}", t.name);
        }
    }

    #[test]
    fn every_task_binary_exists() {
        let bins = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bin");
        for t in std::iter::once(setup()).chain(tasks()) {
            assert!(
                bins.join(format!("{}.rs", t.bin)).exists(),
                "{}: no src/bin/{}.rs",
                t.name,
                t.bin
            );
        }
    }

    #[test]
    fn expected_failures_have_no_output_checks() {
        for t in tasks() {
            if matches!(t.exit, Exit::Fails { .. }) {
                assert!(
                    t.checks.is_empty(),
                    "{}: checks on a task expected to fail never run",
                    t.name
                );
            }
        }
    }

    #[test]
    fn measures_work_on_a_synthetic_image() {
        let mut img = image::RgbaImage::from_pixel(100, 100, image::Rgba([255, 255, 255, 255]));
        for y in 40..60 {
            for x in 40..60 {
                img.put_pixel(x, y, image::Rgba([31, 119, 180, 255]));
            }
        }
        assert!(
            Measure::NotBlank { min_fraction: 0.03 }
                .evaluate(&img)
                .passed
        );
        assert!(
            Measure::Hue {
                region: Region::ALL,
                rgb: BLUE,
                min_px: 400
            }
            .evaluate(&img)
            .passed
        );
        assert!(
            Measure::Colour {
                region: Region::ALL,
                rgb: BLUE,
                tol: 2,
                min_px: 400
            }
            .evaluate(&img)
            .passed
        );
        assert!(
            !Measure::NoColour {
                region: Region::ALL,
                max_px: 0
            }
            .evaluate(&img)
            .passed
        );
        assert!(
            Measure::Runs {
                y: 0.5,
                x0: 0.0,
                x1: 1.0,
                min: 1,
                max: 1
            }
            .evaluate(&img)
            .passed
        );
        assert!(
            !Measure::Ink {
                region: Region::new(0.0, 0.0, 0.3, 0.3),
                min_px: 1
            }
            .evaluate(&img)
            .passed
        );
    }
}
