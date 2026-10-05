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

    /// A WGSL library module or generated glue module failed to compose.
    /// The message is naga_oil's codespan report, mapped back to the
    /// authored module.
    #[error("shader composition failed for {module}:\n{report}")]
    Compose {
        /// The module (import path or glue signature) being composed.
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
