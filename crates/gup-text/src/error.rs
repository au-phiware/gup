// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The crate's error type.

/// Errors produced by `gup-text`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Font data could not be parsed.
    #[error("font {name:?} could not be loaded: {reason}")]
    Font {
        /// The name the caller gave the font.
        name: String,
        /// The parser's reason.
        reason: String,
    },

    /// The glyph atlas is at the device's largest 2D texture size and has
    /// no room for another glyph.
    #[error(
        "glyph atlas is full: {glyphs} glyphs fill a {size}x{size} atlas (the device's \
         largest 2D texture) and {ch:?} at {px} px does not fit; draw fewer distinct \
         sizes or create a new TextSystem"
    )]
    AtlasFull {
        /// The glyph that did not fit.
        ch: char,
        /// Its size in physical pixels.
        px: f32,
        /// The atlas's width and height.
        size: u32,
        /// Glyphs already in the atlas.
        glyphs: usize,
    },
}

/// `Result` with [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
