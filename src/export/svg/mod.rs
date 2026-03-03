// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! SVG export for Gup charts.
//!
//! This module provides an [`SvgRenderer`] that traverses chart data —
//! mark instances, axis geometry, grid lines, titles — and produces a
//! well-formed SVG document string.  The renderer handles coordinate
//! transformation from GPU clip-space (Y-up, `[-1, 1]`) to SVG viewport
//! coordinates (Y-down, `[0, width] × [0, height]`).
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use gup::export::svg::{SvgExportOptions, SvgRenderer};
//!
//! let options = SvgExportOptions::new(800, 600);
//! let renderer = SvgRenderer::new(options);
//! // renderer.render(&chart) produces a String containing a valid SVG document.
//! ```

pub mod element;
mod renderer;

pub use element::SvgElement;
pub use renderer::{SvgExportOptions, SvgRenderer, write_svg_to_file};
