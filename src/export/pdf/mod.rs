// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! PDF export module.
//!
//! This module converts the SVG element intermediate representation
//! produced by [`super::svg`] into a PDF document using the `printpdf`
//! crate.  It supports configurable page sizes (A4, US Letter, or
//! custom dimensions), portrait/landscape orientation, margins, and
//! multi-page documents.
//!
//! # Feature Gate
//!
//! All types in this module are gated behind the **`pdf`** Cargo feature
//! and are not available in the default build.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use gup::export::pdf::{PdfOptions, PdfDocument};
//!
//! let mut doc = PdfDocument::new(PdfOptions::a4());
//! doc.add_page_from_elements("Chart 1", &svg_elements)?;
//! doc.write("report.pdf")?;
//! ```

mod options;
mod renderer;

pub use options::{Orientation, PdfOptions};
pub use renderer::{PdfDocument, PdfRenderer};
