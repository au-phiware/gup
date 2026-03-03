// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Export functionality for producing non-GPU output from Gup charts.
//!
//! This module provides renderers that convert chart data into static
//! file formats such as SVG, enabling resolution-independent, editable
//! output without requiring a GPU.
//!
//! # Available Exporters
//!
//! * [`svg`] — Produces well-formed SVG documents from chart data.

pub mod svg;

pub use svg::{SvgElement, SvgExportOptions, SvgRenderer};
