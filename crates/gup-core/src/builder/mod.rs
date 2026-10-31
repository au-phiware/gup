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
//!     .color(|c| c.region.clone())
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
//! - Plot-level setters (`title`, `subtitle`, `x_title`, `y_title`, `theme`, `grid`,
//!   `legends`, the axis scales) apply to the inner plot at once and work
//!   at any time.
//! - Layer setters (positions, colour, size, ...) are recorded and applied
//!   when the chart is first resolved, handled, picked or its plot or layer
//!   is borrowed. Afterwards they panic; change a drawn chart through
//!   `layer_mut()`.
//! - Colour and size are an accessor plus a scale, combined at that point,
//!   so a scale setter works before or after its accessor.
//! - Rows are owned (`impl IntoIterator<Item = T>`).
//! - Category keys are owned: see [Category keys](#category-keys).
//!
//! # Category keys
//!
//! The category setters (`color` on the scatter, line, area, bar and histogram builders, the
//! area's `stack`, and the bar's `x`, `group` and `stack`) take a closure returning an owned
//! key: any `Hash + Eq + Clone + Display` value, such as an integer, an enum
//! that implements `Display`, `r.name.clone()` or a `&'static str`. A
//! closure returning `r.name.as_str()` does not compile (rustc reports a
//! lifetime error): return `r.name.clone()`.
//!
//! Keys are compared by their label, their `Display` text: two keys that
//! print the same are one category, one colour and one series. Categories
//! are in first-seen order.
//!
//! ```
//! use std::fmt;
//! use gup_core::{ChartExt, Context};
//! # fn main() -> gup_core::Result<()> {
//! #[derive(Clone, Copy, PartialEq, Eq, Hash)]
//! enum Region { North, South }
//! impl fmt::Display for Region {
//!     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//!         f.write_str(match self { Region::North => "North", Region::South => "South" })
//!     }
//! }
//! struct Sale { year: i32, region: Region, total: f64 }
//! let mut rows = Vec::new();
//! for year in [2021, 2022, 2023] {
//!     rows.push(Sale { year, region: Region::North, total: 3.0 });
//!     rows.push(Sale { year, region: Region::South, total: 2.0 });
//! }
//!
//! let mut chart = gup_core::bar(rows)
//!     .x(|s| s.year)
//!     .y(|s| s.total)
//!     .group(|s| s.region);
//!
//! # let cx = Context::new_blocking()?;
//! let (_image, layout) = chart.render_resolved(&cx, 480, 320)?;
//! for label in ["2021", "2022", "2023", "North", "South"] {
//!     assert!(layout.texts.iter().any(|t| &*t.run.text == label), "{label}");
//! }
//! # Ok(())
//! # }
//! ```
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
//! | `.color(Categorical/String)` | `.color(\|d\| d.key.clone())` |
//! | `.color(Float)` + `.color_scale(..)` | `.color_value(\|d\| d.v)` + `.color_value_scale(Sequential::..)` |
//! | `.color(AccessorValue::Color(..))` per row | dropped: map keys through `.color_scale(Categorical::from_palette(..))` |
//! | `.fill_color(c)` / `.stroke_color(c)` | `.fill(c)` / `.stroke(c)` |
//! | `.size(..)` (percent of plot width) | `.size(\|d\| d.w)` (square-root area, 2-12 px) or `.radius(px)` |
//! | `.point_size(px)` | `.radius(px)` |
//! | `.opacity(a)` / `.fill_opacity(a)` | `.opacity(a)` |
//! | `.stroke_width(..)` / `.stroke_width_px(px)` (line) | `.width(px)` |
//! | `.connect_nulls(false)` | default: nulls (`None`, NaN) break the line |
//! | `.connect_nulls(true)` / `.sort_x()` | dropped: filter or sort the rows |
//! | `.interpolate`, `.step`, `.smooth`, `.monotone` | `.curve(Curve::StepAfter)` (also `Step`, `StepBefore`); `.curve(Curve::MonotoneX)` for both smooth curves |
//! | (no old histogram) | `gup_core::histogram(rows).x(\|d\| d.v).bins(Bins::Width(5.0))` |
//! | (no old area) | `gup_core::area(rows).x(\|d\| d.t).y(\|d\| d.v).stack(\|d\| d.k.clone())` |
//! | `.group_by(..)` / `.stack_by(..)` / `.stack(..)` | `.group(\|d\| d.k.clone())` / `.stack(\|d\| d.k.clone())` |
//! | `.gap(g)` / `.bar_width(..)` | `.padding(p)` / dropped: the band decides |
//! | `.orient(..)` / `.horizontal()` | `gup_core::barh(rows)` |
//! | `.anti_alias(..)` / `.point_budget(..)` | dropped: analytic antialiasing always; LOD is T7 |
//! | `.title(..)` | `.title(..)` |
//! | `.width(w)` / `.height(h)` (canvas) | the size argument of `save_png`, `render` or `GupApp::size` |
//! | `.show_axes(true)` | always on |
//! | `.grid()` / `.horizontal_grid()` | `.grid(Axes::Both)` / `.grid(Axes::Y)` |
//! | `.x_tick_format(..)` | `.x_tick_format(\|v\| ..)`: the closure gets the tick value and returns its label |
//! | `.hover_reveal(..)` / `.tooltip_config(..)` | `.tooltip(\|d\| format!(..))` |
//! | `export_png(path, w, h)` | `chart.save_png(&cx, path, w, h)` |
//! | `render_to_rgba(w, h)` | `chart.render(&cx, w, h)` |
//! | `export_svg(..)` | `SvgTarget` over `Scene::guides` (marks not yet) |
//! | `composite().layer(a).layer(b)` | one `Plot` with `circles(..)`, `line(..)` and `rects(..)` layers |
//! | `.layer_with_y2(..)` | `Plot::y_right` (RFC-001 S11) |

use std::any::Any;
use std::cell::RefCell;
use std::sync::Arc;

use crate::column::DictionaryKey;

pub(crate) mod area;
pub(crate) mod bar;
pub(crate) mod boxplot;
pub(crate) mod heatmap;
pub(crate) mod histogram;
pub(crate) mod line;
pub(crate) mod scatter;

pub use area::{AreaChart, AreaPoint, area};
pub use bar::{BarChart, Horizontal, Orientation, Vertical, bar, barh};
pub use boxplot::{BoxPlotChart, BoxSummary, boxplot};
pub use heatmap::{HeatmapChart, heatmap};
pub use histogram::{HistogramBin, HistogramChart, histogram};
pub use line::{LineChart, line};
pub use scatter::{ScatterChart, scatter};

type VisitFn<T> = dyn Fn(&T, &mut dyn FnMut(&str)) + Send + Sync;

/// A category key accessor, erased to the key's label.
pub(crate) struct Key<T>(Arc<VisitFn<T>>);

impl<T> Clone for Key<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T> Key<T> {
    /// Call `f` with the row's label: no allocation for `String`,
    /// `&'static str` and `Arc<str>` keys; anything else is formatted
    /// into a reused thread-local `String`.
    pub(crate) fn visit(&self, row: &T, f: &mut dyn FnMut(&str)) {
        (self.0)(row, f);
    }

    /// The label as an `Arc<str>` (allocates), for the builders' own CPU
    /// passes (stack sums, categories, label rows).
    pub(crate) fn label(&self, row: &T) -> Arc<str> {
        let mut out = None;
        self.visit(row, &mut |s| out = Some(Arc::from(s)));
        out.expect("visit calls its callback once")
    }
}

/// Erase an owned key accessor to one returning the key's label.
pub(crate) fn erase_key<T, K: DictionaryKey>(
    key: impl Fn(&T) -> K + Send + Sync + 'static,
) -> Key<T> {
    Key(Arc::new(move |row, f| visit_label(&key(row), f)))
}

thread_local! {
    /// Where keys without a string form are formatted.
    static LABEL_BUF: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Call `f` with a key's label: borrowed for `Arc<str>`, `String` and
/// `&'static str`, formatted through `Display` otherwise.
fn visit_label<K: DictionaryKey>(key: &K, f: &mut dyn FnMut(&str)) {
    let any = key as &dyn Any;
    if let Some(s) = any.downcast_ref::<Arc<str>>() {
        f(s);
    } else if let Some(s) = any.downcast_ref::<String>() {
        f(s);
    } else if let Some(s) = any.downcast_ref::<&'static str>() {
        f(s);
    } else {
        // A `Display` that re-enters (formats another key) finds the buffer
        // taken and falls back to a fresh string.
        LABEL_BUF.with(|buf| match buf.try_borrow_mut() {
            Ok(mut buf) => {
                use std::fmt::Write;
                buf.clear();
                let _ = write!(buf, "{key}");
                f(&buf);
            }
            Err(_) => f(&key.to_string()),
        });
    }
}

/// The message of the panic when a layer setter is called on a chart that
/// has been drawn.
pub(crate) const DRAWN: &str =
    "set layer encodings before the chart is first drawn; change a drawn chart through layer_mut()";

/// The plot-level setters every builder shares, delegating to `self.plot`.
macro_rules! plot_setters {
    () => {
        $crate::builder::plot_setters!(titles);

        /// Label the x axis ticks with `f` instead of the default labels.
        /// `f` receives the tick value in the scale's domain units (Unix
        /// seconds on a [`Time`](crate::Time) axis) and returns its label.
        /// It works before or after `x_scale`; the last format set wins.
        /// A band or point axis is labelled by its keys, so drawing
        /// fails with a config error.
        ///
        /// ```no_run
        /// # use gup_core::prelude::*;
        /// let chart = gup_core::scatter([(1200.0, 2.0), (45000.0, 3.0)])
        ///     .x(|p| p.0)
        ///     .x_scale(Log::new())
        ///     .x_tick_format(|v| format!("${:.0}k", v / 1000.0))
        ///     .y(|p| p.1);
        /// ```
        pub fn x_tick_format(mut self, f: impl Fn(f64) -> String + Send + Sync + 'static) -> Self {
            self.plot.x_tick_format(f);
            self
        }

        /// Label the y axis ticks with `f`; see `x_tick_format`.
        pub fn y_tick_format(mut self, f: impl Fn(f64) -> String + Send + Sync + 'static) -> Self {
            self.plot.y_tick_format(f);
            self
        }
    };
    (titles) => {
        /// Set the chart title.
        pub fn title(mut self, title: impl Into<String>) -> Self {
            self.plot.title(title);
            self
        }

        /// Set the chart subtitle, shown under the title in lighter text.
        pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
            self.plot.subtitle(subtitle);
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

#[cfg(test)]
mod send_sync {
    fn assert_send_sync<T: Send + Sync>() {}

    /// Hosts that keep charts in components or resources (Bevy) need
    /// `Send + Sync`, as `Plot` has.
    #[test]
    fn builders_are_send_and_sync() {
        assert_send_sync::<crate::Plot>();
        assert_send_sync::<super::ScatterChart<(f64, f64)>>();
        assert_send_sync::<super::LineChart<(f64, f64)>>();
        assert_send_sync::<super::AreaChart<(f64, f64)>>();
        assert_send_sync::<super::BarChart<(f64, f64)>>();
        assert_send_sync::<super::BoxPlotChart<(f64, f64)>>();
        assert_send_sync::<super::HistogramChart<f64>>();
        assert_send_sync::<super::HeatmapChart<(f64, f64)>>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;

    #[derive(Clone, PartialEq, Eq, Hash)]
    enum Region {
        North,
    }

    impl fmt::Display for Region {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(match self {
                Self::North => "Northern",
            })
        }
    }

    /// A key whose `Display` formats another key through the same buffer.
    #[derive(Clone, PartialEq, Eq, Hash)]
    struct Nested(u32);

    impl fmt::Display for Nested {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let inner = erase_key(|n: &u32| *n);
            inner.visit(&self.0, &mut |s| {
                let _ = write!(f, "<{s}>");
            });
            Ok(())
        }
    }

    fn agree<K: DictionaryKey>(key: K, want: &str) {
        let k = erase_key(move |_: &()| key.clone());
        let mut seen = String::new();
        k.visit(&(), &mut |s| seen.push_str(s));
        assert_eq!(seen, want);
        assert_eq!(&*k.label(&()), want);
    }

    #[test]
    fn label_and_visit_agree_for_every_key_kind() {
        agree(String::from("a b"), "a b");
        agree("lit", "lit");
        agree(Arc::<str>::from("shared"), "shared");
        agree(7_u32, "7");
        agree(Region::North, "Northern");
    }

    #[test]
    fn a_display_that_formats_a_key_does_not_panic() {
        agree(Nested(5), "<5>");
    }
}
