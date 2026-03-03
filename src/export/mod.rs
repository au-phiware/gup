// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Export functionality for producing non-GPU output from Gup charts.
//!
//! This module provides renderers that convert chart data into static
//! file formats such as SVG, PDF, and PNG, enabling resolution-independent
//! or pixel-perfect output.
//!
//! # Available Exporters
//!
//! * [`svg`] — Produces well-formed SVG documents from chart data.
//! * [`pdf`] — Converts the SVG intermediate to PDF (requires the `pdf` feature).
//! * [`png`] — Renders to an off-screen GPU texture and encodes as PNG.

pub mod png;
pub mod svg;

#[cfg(feature = "pdf")]
pub mod pdf;

pub use svg::write_svg_to_file;
pub use svg::{SvgElement, SvgExportOptions, SvgRenderer};

#[cfg(feature = "pdf")]
pub use pdf::{Orientation as PdfOrientation, PdfDocument, PdfOptions, PdfRenderer};
