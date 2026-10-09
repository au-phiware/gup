// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gup's text system (RFC-001 S2).
//!
//! One [`TextSystem`] per device owns a [`Font`] and a glyph atlas. It
//! measures text ([`TextSystem::measure`], [`TextSystem::ink_bounds`]),
//! lays out runs into [`Glyphs`] and prepares a [`GlyphBatch`] that records
//! its draws into a render pass the caller already has open, so text draws
//! in the same pass as everything else.
//!
//! - **Font**: a subset of Inter Regular is bundled ([`Font::inter`], SIL
//!   Open Font License 1.1, 30 KB gzipped): Latin (Western, Central and
//!   Eastern European), basic Greek, and the digits, currency, maths and
//!   punctuation charts use ([`INTER_REGULAR`] lists it). The complete face
//!   is [`Font::inter_full`], and any TrueType or OpenType face loads with
//!   [`Font::from_bytes`]. A character a font lacks draws as its visible
//!   missing-glyph box; [`Font::missing_glyphs`] finds them.
//! - **Rasterisation**: glyphs are coverage bitmaps rasterised at their
//!   physical pixel size and drawn 1:1 on whole pixels, so text is sharp at
//!   any device pixel ratio. There is no MSDF, rotation or wrapping yet.
//! - **Atlas**: one R8 texture, shelf-packed, doubling when full up to the
//!   device's largest 2D texture. Only the rectangle that changed since the
//!   last upload is written.
//! - **Uploads** go through an [`Uploader`], so a host can count them.
//!
//! ```no_run
//! use gup_text::{Anchor, DrawTarget, GlyphBuffer, Glyphs, HAlign, Run, TextSystem, VAlign};
//!
//! # fn demo(device: &wgpu::Device, queue: &wgpu::Queue, pass: &mut wgpu::RenderPass<'_>)
//! # -> gup_text::Result<()> {
//! let mut text = TextSystem::new(device);
//! let run = Run {
//!     text: "Hello",
//!     size: 16.0,
//!     at: [100.0, 20.0],
//!     anchor: Anchor::new(HAlign::Middle, VAlign::Top),
//! };
//! let width = text.measure(run.text, run.size).width;
//! # let _ = width;
//!
//! let mut glyphs = Glyphs::new(1.0);
//! text.layout(&run, [0.0, 0.0, 0.0, 1.0], &mut glyphs)?;
//! let target = DrawTarget {
//!     format: wgpu::TextureFormat::Rgba8Unorm,
//!     samples: 1,
//!     width: 200,
//!     height: 100,
//! };
//! let mut buffer = GlyphBuffer::new();
//! if let Some(batch) = text.prepare(queue, &mut buffer, &glyphs, &target) {
//!     batch.draw(pass);
//! }
//! # Ok(())
//! # }
//! ```

mod atlas;
mod error;
mod font;
mod layout;
mod system;

pub use error::{Error, Result};
pub use font::{Font, INTER_REGULAR, INTER_REGULAR_FULL, LineMetrics};
pub use layout::{Anchor, Bounds, HAlign, Run, TextMetrics, VAlign};
pub use system::{DrawTarget, GlyphBatch, GlyphBuffer, Glyphs, TextSystem, Uploader};
/// The wgpu `gup-text` is built against.
pub use wgpu;
