// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chart builders: one call per encoding, default scales, a finished chart.
//!
//! A builder is a [`Chart`](crate::Chart) that owns a [`Plot`](crate::Plot)
//! and is sugar over its adders and scales. The north-star scatter:
//!
//! ```
//! use gup_core::{ChartExt, Context};
//! # fn main() -> gup_core::Result<()> {
//! struct Country { gdp: f64, life: f64, region: String, people: f64 }
//! let countries = vec![
//!     Country { gdp: 1e3, life: 55.0, region: "Africa".into(), people: 3e7 },
//!     Country { gdp: 2e4, life: 74.0, region: "Europe".into(), people: 6e7 },
//!     Country { gdp: 6e4, life: 81.0, region: "Asia".into(), people: 1e9 },
//! ];
//!
//! let mut chart = gup_core::scatter(countries)
//!     .x(|c| c.gdp)
//!     .y(|c| c.life)
//!     .color(|c| c.region.as_str())
//!     .size(|c| c.people);
//!
//! # let cx = Context::new_blocking()?;
//! let image = chart.render(&cx, 320, 200)?;
//! assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
//! # Ok(())
//! # }
//! ```
//!
//! # Pattern
//!
//! - Setters take and return `self`, so a chain on a temporary works;
//!   rendering needs `let mut chart`.
//! - Plot-level setters (`title`, `x_title`, `y_title`, `theme`, `grid`,
//!   `legends`, the axis scales) apply to the inner plot at once and work
//!   at any time.
//! - Layer setters (positions, colour, size, ...) are recorded and applied
//!   when the chart is first resolved, handled, picked or its plot or layer
//!   is borrowed. Afterwards they panic; change a drawn chart through
//!   `layer_mut()`.
//! - Colour and size are an accessor plus a scale, combined at that point,
//!   so a scale setter works before or after its accessor.
//! - Rows are owned (`impl IntoIterator<Item = T>`).
//!
//! # Showing it
//!
//! Hand a finished chart to [`show`](crate::show) to open a window:
//!
//! ```no_run
//! let chart = gup_core::scatter([(1.0, 2.0), (2.0, 3.0)]).x(|p| p.0).y(|p| p.1);
//! gup_core::show(chart).unwrap();
//! ```
//!
//! # Migrating from the old builders
//!
//! The old builders lived in `gup::chart_builder`.
//!
//! | Old | New |
//! | --- | --- |
//! | `scatter()` / `line()` / `bar()` | `gup_core::scatter(rows)` / `line(rows)` / `bar(rows)` |
//! | `.x(AccessorFunction::new(\|d\| AccessorValue::Float(d.a)))` | `.x(\|d\| d.a)` |
//! | `.build_with_data(rows, ctx)` | rows go to `scatter(rows)`; the context is `Context::shared()` or passed to `save_png` |
//! | `.x_scale(LinearScale::new(d0, d1, -1.0, 1.0))` | `.x_scale(Linear::new().domain(d0, d1))`; omit for an automatic domain |
//! | `LogScale::base10(..)` | `.x_scale(Log::new())` |
//! | `.color(Categorical/String)` | `.color(\|d\| d.key.as_str())` |
//! | `.color(Float)` + `.color_scale(..)` | `.color_value(\|d\| d.v)` + `.color_value_scale(Sequential::..)` |
//! | `.color(AccessorValue::Color(..))` per row | dropped: map keys through `.color_scale(Categorical::from_palette(..))` |
//! | `.fill_color(c)` / `.stroke_color(c)` | `.fill(c)` / `.stroke(c)` |
//! | `.size(..)` (percent of plot width) | `.size(\|d\| d.w)` (square-root area, 2-12 px) or `.radius(px)` |
//! | `.point_size(px)` | `.radius(px)` |
//! | `.opacity(a)` / `.fill_opacity(a)` | `.opacity(a)` |
//! | `.stroke_width(..)` / `.stroke_width_px(px)` (line) | `.width(px)` |
//! | `.connect_nulls(false)` | default: nulls (`None`, NaN) break the line |
//! | `.connect_nulls(true)` / `.sort_x()` | dropped: filter or sort the rows |
//! | `.interpolate`, `.smooth`, `.monotone`, `.step` | dropped: straight segments only |
//! | `.group_by(..)` / `.stack_by(..)` / `.stack(..)` | `.group(\|d\| d.k.as_str())` / `.stack(\|d\| d.k.as_str())` |
//! | `.gap(g)` / `.bar_width(..)` | `.padding(p)` / dropped: the band decides |
//! | `.orient(..)` / `.horizontal()` | dropped: vertical bars only |
//! | `.anti_alias(..)` / `.point_budget(..)` | dropped: analytic antialiasing always; LOD is T7 |
//! | `.title(..)` | `.title(..)` |
//! | `.width(w)` / `.height(h)` (canvas) | the size argument of `save_png`, `render` or `GupApp::size` |
//! | `.show_axes(true)` | always on |
//! | `.grid()` / `.horizontal_grid()` | `.grid(Axes::Both)` / `.grid(Axes::Y)` |
//! | `.x_tick_format(..)` | dropped: `Time` gives calendar labels |
//! | `.hover_reveal(..)` / `.tooltip_config(..)` | `.tooltip(\|d\| format!(..))` |
//! | `export_png(path, w, h)` | `chart.save_png(&cx, path, w, h)` |
//! | `render_to_rgba(w, h)` | `chart.render(&cx, w, h)` |
//! | `export_svg(..)` | `SvgTarget` over `Scene::guides` (marks not yet) |
//! | `composite().layer(a).layer(b)` | one `Plot` with `circles(..)`, `line(..)` and `rects(..)` layers |
//! | `.layer_with_y2(..)` | `Plot::y_right` (RFC-001 S11) |

pub(crate) mod bar;
pub(crate) mod line;
pub(crate) mod scatter;

pub use bar::{BarChart, bar};
pub use line::{LineChart, line};
pub use scatter::{ScatterChart, scatter};

/// The message of the panic when a layer setter is called on a chart that
/// has been drawn.
pub(crate) const DRAWN: &str =
    "set layer encodings before the chart is first drawn; change a drawn chart through layer_mut()";

/// The plot-level setters every builder shares, delegating to `self.plot`.
macro_rules! plot_setters {
    () => {
        /// Set the chart title.
        pub fn title(mut self, title: impl Into<String>) -> Self {
            self.plot.title(title);
            self
        }

        /// Set the x axis title.
        pub fn x_title(mut self, title: impl Into<String>) -> Self {
            self.plot.x_title(title);
            self
        }

        /// Set the y axis title.
        pub fn y_title(mut self, title: impl Into<String>) -> Self {
            self.plot.y_title(title);
            self
        }

        /// Style the chart with `theme`.
        pub fn theme(mut self, theme: crate::Theme) -> Self {
            self.plot.theme(theme);
            self
        }

        /// Draw grid lines at the ticks of `axes` (default none).
        pub fn grid(mut self, axes: crate::Axes) -> Self {
            self.plot.grid(axes);
            self
        }

        /// Draw the colour legends (default) or not.
        pub fn legends(mut self, on: bool) -> Self {
            self.plot.legends(on);
            self
        }
    };
}
pub(crate) use plot_setters;
