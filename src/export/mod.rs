// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Export functionality for producing non-GPU output from Gup charts.
//!
//! This module provides renderers that convert chart data into static
//! file formats such as SVG, PDF, PNG, and self-contained HTML, enabling
//! resolution-independent, pixel-perfect, or shareable output.
//!
//! # Available Exporters
//!
//! * [`svg`] — Produces well-formed SVG documents from chart data.
//! * [`png`] — Renders to an off-screen GPU texture and encodes as PNG.

pub mod png;
pub mod svg;

pub mod gallery;

pub use svg::SvgElement;
