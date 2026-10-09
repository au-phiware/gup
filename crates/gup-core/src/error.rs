// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The crate-wide error type.

/// Errors produced by `gup-core`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No GPU adapter matched the request.
    #[error("no suitable GPU adapter: {0}")]
    Adapter(#[from] wgpu::RequestAdapterError),

    /// The adapter refused to create a device.
    #[error("GPU device request failed: {0}")]
    Device(#[from] wgpu::RequestDeviceError),

    /// A generated top-level module (glue) failed to link against the
    /// library modules flattened at build time. Authored WGSL is composed
    /// at build time, so this is a bug in Gup's generator, not a user error;
    /// the report names the generated line.
    #[error("shader composition failed for {module}:\n{report}")]
    Compose {
        /// The module (glue signature or label) being linked.
        module: String,
        /// The formatted diagnostic.
        report: String,
    },

    /// A chart, layer or channel was configured in a way that cannot be
    /// rendered.
    #[error("invalid {what}: {detail}")]
    Configuration {
        /// The component that is misconfigured.
        what: &'static str,
        /// What is wrong and, where possible, how to fix it.
        detail: String,
    },

    /// The GPU rejected work `gup-core` gave it: a WebGPU validation error,
    /// an out-of-memory error or (natively) an internal error, caught by an
    /// error scope around pipeline creation, resolution or rendering.
    ///
    /// `what` names the step: a pipeline by its label and glue signature,
    /// a layer, or a target. `message` is wgpu's (natively) or the
    /// browser's (on wasm32) message, such as a WGSL diagnostic.
    #[error("GPU error in {what}: {message}")]
    Gpu {
        /// The pipeline, layer or target whose GPU work failed.
        what: String,
        /// The GPU's message.
        message: String,
    },

    /// Waiting for the GPU or mapping a readback buffer failed.
    #[error("GPU readback failed: {0}")]
    Readback(String),

    /// Writing an output file failed.
    #[error("could not write {path}: {source}")]
    Io {
        /// The file being written.
        path: String,
        /// The underlying error.
        source: std::io::Error,
    },

    /// Encoding an image failed.
    #[error("image encoding failed: {0}")]
    Image(#[from] image::ImageError),

    /// Laying out or drawing text failed (for example, the glyph atlas is
    /// full).
    #[error(transparent)]
    Text(#[from] gup_text::Error),
}

impl Error {
    /// A configuration error for `what`, with a contextual `detail`.
    pub(crate) fn config(what: &'static str, detail: impl Into<String>) -> Self {
        Self::Configuration {
            what,
            detail: detail.into(),
        }
    }
}

/// `Result` with [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
