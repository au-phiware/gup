// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery screenshot support.
//!
//! Provides a helper that examples call to detect the
//! `GUP_SCREENSHOT_PATH` environment variable.  When the variable is set
//! the example should render a single frame offscreen, export it to the
//! specified path, and exit.
//!
//! The optional `GUP_SCREENSHOT_WIDTH` and `GUP_SCREENSHOT_HEIGHT`
//! variables allow the caller (typically `scripts/generate_gallery.sh`)
//! to override the thumbnail dimensions.  Defaults are 640×480.

use std::path::PathBuf;

/// The default thumbnail width in pixels.
pub const DEFAULT_WIDTH: u32 = 640;
/// The default thumbnail height in pixels.
pub const DEFAULT_HEIGHT: u32 = 480;

/// Parsed screenshot request from environment variables.
#[derive(Debug, Clone)]
pub struct ScreenshotRequest {
    /// Destination path for the PNG file.
    pub path: PathBuf,
    /// Thumbnail width in pixels.
    pub width: u32,
    /// Thumbnail height in pixels.
    pub height: u32,
}

/// Check whether a gallery screenshot has been requested via the environment.
///
/// Returns `Some(ScreenshotRequest)` when `GUP_SCREENSHOT_PATH` is set,
/// or `None` otherwise.
///
/// # Example
///
/// ```no_run
/// if let Some(req) = gup::export::gallery::screenshot_request() {
///     // render one frame offscreen and save to req.path
/// }
/// ```
pub fn screenshot_request() -> Option<ScreenshotRequest> {
    let path = std::env::var("GUP_SCREENSHOT_PATH").ok()?;
    let width = std::env::var("GUP_SCREENSHOT_WIDTH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_WIDTH);
    let height = std::env::var("GUP_SCREENSHOT_HEIGHT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_HEIGHT);
    Some(ScreenshotRequest {
        path: PathBuf::from(path),
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_when_env_unset() {
        // The variable is not set in the normal test environment.
        // SAFETY: Tests run single-threaded (--test-threads=1).
        unsafe { std::env::remove_var("GUP_SCREENSHOT_PATH") };
        assert!(screenshot_request().is_none());
    }

    #[test]
    fn parses_env_with_defaults() {
        // SAFETY: Tests run single-threaded (--test-threads=1).
        unsafe {
            std::env::set_var("GUP_SCREENSHOT_PATH", "/tmp/test.png");
            std::env::remove_var("GUP_SCREENSHOT_WIDTH");
            std::env::remove_var("GUP_SCREENSHOT_HEIGHT");
        }

        let req = screenshot_request().expect("should return Some");
        assert_eq!(req.path, PathBuf::from("/tmp/test.png"));
        assert_eq!(req.width, DEFAULT_WIDTH);
        assert_eq!(req.height, DEFAULT_HEIGHT);

        // Clean up.
        // SAFETY: Tests run single-threaded (--test-threads=1).
        unsafe { std::env::remove_var("GUP_SCREENSHOT_PATH") };
    }

    #[test]
    fn parses_custom_dimensions() {
        // SAFETY: Tests run single-threaded (--test-threads=1).
        unsafe {
            std::env::set_var("GUP_SCREENSHOT_PATH", "/tmp/custom.png");
            std::env::set_var("GUP_SCREENSHOT_WIDTH", "320");
            std::env::set_var("GUP_SCREENSHOT_HEIGHT", "240");
        }

        let req = screenshot_request().expect("should return Some");
        assert_eq!(req.width, 320);
        assert_eq!(req.height, 240);

        // SAFETY: Tests run single-threaded (--test-threads=1).
        unsafe {
            std::env::remove_var("GUP_SCREENSHOT_PATH");
            std::env::remove_var("GUP_SCREENSHOT_WIDTH");
            std::env::remove_var("GUP_SCREENSHOT_HEIGHT");
        }
    }
}
