// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The S0 reference scatter in a window: scroll to zoom about the cursor,
//! Escape to close.
//!
//! ```text
//! cargo run -p gup-core --example scatter_window
//! ```
//!
//! With `GUP_SCREENSHOT_PATH` set it draws one frame in the window, writes
//! the window's pixels as PNG and exits (`GUP_SCREENSHOT_WIDTH` and
//! `GUP_SCREENSHOT_HEIGHT` set the size; 720×450 matches the golden image
//! `tests/golden/gup_core/scatter.png`).

#[path = "../tests/common/scatter.rs"]
mod scatter;

fn main() -> gup_core::Result<()> {
    gup_core::show(scatter::plot())
}
