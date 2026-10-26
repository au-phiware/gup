// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # gup
//!
//! GPU-accelerated data visualization. This crate is the public face of
//! [`gup_core`]: everything `gup-core` exports (its root items, the
//! [`prelude`], `wgpu`, the shader-function macros) is re-exported here, and
//! this crate adds nothing of its own.
//!
//! ## A scatter chart in a few lines
//!
//! ```
//! use gup::prelude::*;
//!
//! struct Row { gdp: f64, life_exp: f64, continent: String, population: f64 }
//! let data = vec![
//!     Row { gdp: 1e3, life_exp: 55.0, continent: "Africa".into(), population: 3e7 },
//!     Row { gdp: 2e4, life_exp: 74.0, continent: "Europe".into(), population: 6e7 },
//!     Row { gdp: 6e4, life_exp: 81.0, continent: "Asia".into(), population: 1e9 },
//! ];
//!
//! let mut chart = gup::scatter(data)
//!     .x(|d| d.gdp)
//!     .y(|d| d.life_exp)
//!     .color(|d| d.continent.clone())
//!     .size(|d| d.population);
//! let image = chart.render(&Context::shared()?, 800, 500)?;
//! assert_eq!(image.dimensions(), (800, 500));
//! # Ok::<(), gup::Error>(())
//! ```
//!
//! ## A histogram in one line
//!
//! `gup::histogram` counts a column into bins that start and end on round
//! numbers; [`Bins`] (in the prelude) chooses them.
//!
//! ```
//! use gup::prelude::*;
//!
//! struct Trip { minutes: f64 }
//! let trips = (0..300).map(|i| Trip { minutes: f64::from(i % 60) });
//!
//! let mut chart = gup::histogram(trips).x(|t| t.minutes).bins(Bins::Width(10.0));
//! // Six bins of ten minutes: 0-10, 10-20, ... 50-60.
//! assert_eq!(chart.layers_mut()[0].len(), 6);
//! let image = chart.render(&Context::shared()?, 480, 320)?;
//! assert_eq!(image.dimensions(), (480, 320));
//! # Ok::<(), gup::Error>(())
//! ```
//!
//! ## Step lines
//!
//! A price set at times holds until it changes: [`Curve::StepAfter`] draws
//! it level from each row to the next row's x, then upright. The same
//! setter is on a line layer ([`Selection::curve`]).
//!
//! ```
//! use gup::prelude::*;
//!
//! struct Price { t: f64, price: f64 }
//! let prices = vec![
//!     Price { t: 0.0, price: 10.0 },
//!     Price { t: 2.0, price: 12.5 },
//!     Price { t: 3.0, price: 11.0 },
//!     Price { t: 6.0, price: 14.0 },
//! ];
//!
//! let mut chart = gup::line(prices).x(|p| p.t).y(|p| p.price).curve(Curve::StepAfter);
//! let image = chart.render(&Context::shared()?, 480, 320)?;
//! assert_eq!(image.dimensions(), (480, 320));
//!
//! // A layer of a plot takes the same setter.
//! let mut plot = Plot::new();
//! let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
//! plot.add(Selection::<(f64, f64), Line>::new(vec![(0.0, 1.0), (1.0, 3.0), (2.0, 2.0)]))
//!     .x_with(x.encode(|r: &(f64, f64)| r.0))
//!     .y_with(y.encode(|r: &(f64, f64)| r.1))
//!     .curve(Curve::Step);
//! let (image, _) = plot.render_resolved(&Context::shared()?, 320, 200)?;
//! assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
//! # Ok::<(), gup::Error>(())
//! ```
//!
//! ## Labels at data positions
//!
//! `plot.text(rows)` adds a layer of labels, placed through the plot's
//! scales like any layer. `Anchor::BOTTOM` puts each label's bottom-centre
//! on its point, so `.dy(-6.0)` lifts it 6 px clear of the disc.
//!
//! ```
//! use gup::prelude::*;
//! use gup::scene::TextRole;
//!
//! #[derive(Clone)]
//! struct City { name: &'static str, lon: f64, lat: f64 }
//! let cities = vec![
//!     City { name: "Perth", lon: 115.9, lat: -31.9 },
//!     City { name: "Darwin", lon: 130.8, lat: -12.5 },
//!     City { name: "Sydney", lon: 151.2, lat: -33.9 },
//! ];
//!
//! let mut plot = Plot::new();
//! let x = plot.x(Linear::new().domain(110.0, 160.0));
//! let y = plot.y(Linear::new().domain(-45.0, -5.0));
//! plot.circles(cities.clone()).x(|c| c.lon).y(|c| c.lat).radius(Px(4.0));
//! plot.text(cities.clone())
//!     .x(|c| c.lon)
//!     .y(|c| c.lat)
//!     .text(|c| c.name)
//!     .anchor(Anchor::BOTTOM)
//!     .dy(-6.0);
//!
//! let layout = &plot.resolve(&Context::shared()?, 480.0, 320.0)?.layout;
//! for city in &cities {
//!     let label = layout
//!         .texts
//!         .iter()
//!         .find(|t| t.run.role == TextRole::Annotation && &*t.run.text == city.name)
//!         .expect("every city is labelled");
//!     let (px, py) = (x.read().eval(city.lon) as f32, y.read().eval(city.lat) as f32);
//!     // The ink is above the point, at least the 6 px offset, and centred on it.
//!     assert!(label.bounds.bottom() <= py - 5.5);
//!     let centre = label.bounds.x + label.bounds.width / 2.0;
//!     assert!((centre - px).abs() < 2.0);
//! }
//! # Ok::<(), gup::Error>(())
//! ```
//!
//! ## Your own shader function
//!
//! `#[gup::wgsl_function]` turns WGSL into a typed shader function. Here a
//! colour ramp colours each circle. A doctest is an external crate that
//! depends on `gup` alone; `path` is set because its package is named `gup`.
//!
//! ```
//! use gup::prelude::*;
//!
//! /// Blue for low values, orange-red for high ones.
//! #[derive(Clone, Debug)]
//! struct Ramp;
//!
//! #[gup::wgsl_function(path = "ramp_demo", wgsl = r#"
//!     struct Params { pad: f32 }
//!
//!     fn ramp(v: f32, p: Params) -> vec4<f32> {
//!         return vec4<f32>(v, 0.3, 1.0 - v, 1.0);
//!     }
//! "#)]
//! impl ShaderFn for Ramp {
//!     fn params(&self) -> RampParams {
//!         RampParams { pad: 0.0 }
//!     }
//! }
//!
//! struct Reading { x: f64, y: f64, v: f64 }
//! let readings: Vec<Reading> = (0..16)
//!     .map(|i| Reading { x: f64::from(i % 4), y: f64::from(i / 4), v: f64::from(i) / 15.0 })
//!     .collect();
//!
//! let cx = Context::shared()?;
//! let mut plot = Plot::new();
//! plot.x(Linear::new().domain(-1.0, 4.0));
//! plot.y(Linear::new().domain(-1.0, 4.0));
//! plot.circles(readings)
//!     .x(|r: &Reading| r.x)
//!     .y(|r: &Reading| r.y)
//!     .attr(Circle::FILL, Ramp.encode(|r: &Reading| r.v));
//! let (image, _) = plot.render_resolved(&cx, 320, 320)?;
//! assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
//! # Ok::<(), gup::Error>(())
//! ```

#![warn(missing_docs)]

pub use gup_core::*;

/// The architecture documents' snippets, compiled as doctests.
#[cfg(doctest)]
mod architecture_docs {
    #[doc = include_str!("../docs/TECHNICAL_APPROACH.md")]
    struct TechnicalApproach;
    #[doc = include_str!("../docs/IMPLEMENTATION_STRATEGY.md")]
    struct ImplementationStrategy;
    #[doc = include_str!("../README.md")]
    struct Readme;
}

/// The tutorials' snippets, compiled as doctests (GUP-530).
#[cfg(doctest)]
mod tutorials {
    /// Tutorial 1, "Getting Started" (GUP-530).
    #[doc = include_str!("../docs/tutorials/01_getting_started.md")]
    struct GettingStarted;
    /// Tutorial 2, "Plots and Selections" (GUP-530).
    #[doc = include_str!("../docs/tutorials/02_data_binding.md")]
    struct PlotsAndSelections;
    /// Tutorial 3, "Custom Shader Functions" (GUP-531).
    #[doc = include_str!("../docs/tutorials/03_custom_shader_functions.md")]
    struct ShaderFunctions;
    /// Tutorial 4, "Interactions" (GUP-532).
    #[doc = include_str!("../docs/tutorials/04_interactions.md")]
    struct Interaction;
    /// Tutorial 5, "Streaming Data" (GUP-532).
    #[doc = include_str!("../docs/tutorials/05_streaming_data.md")]
    struct Streaming;
    /// Tutorial 6, "Custom Marks" (GUP-531).
    #[doc = include_str!("../docs/tutorials/06_custom_marks.md")]
    struct CustomMarks;
}
