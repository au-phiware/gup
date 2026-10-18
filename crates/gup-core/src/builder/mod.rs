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

pub(crate) mod scatter;

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
