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
}
