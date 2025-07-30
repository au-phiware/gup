// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
//! Error handling for Gup visualization library.

use std::fmt;

/// Main error type for Gup operations.
#[derive(Debug, Clone)]
pub enum GupError {
    /// Rendering operation failed
    RenderError(String),
    /// Composition operation failed
    CompositionError(String),
    /// GPU resource allocation failed
    ResourceError(String),
    /// Invalid operation or parameters
    InvalidOperation(String),
    /// WebGPU-specific error
    WebGpuError(String),
}

impl fmt::Display for GupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GupError::RenderError(msg) => write!(f, "Render error: {msg}"),
            GupError::CompositionError(msg) => write!(f, "Composition error: {msg}"),
            GupError::ResourceError(msg) => write!(f, "Resource error: {msg}"),
            GupError::InvalidOperation(msg) => write!(f, "Invalid operation: {msg}"),
            GupError::WebGpuError(msg) => write!(f, "WebGPU error: {msg}"),
        }
    }
}

impl std::error::Error for GupError {}

/// Result type alias for Gup operations.
pub type GupResult<T> = Result<T, GupError>;
