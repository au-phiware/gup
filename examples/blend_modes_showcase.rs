// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
//! Blend Modes Showcase Example Binary
//!
//! Run with: `cargo run --example blend_modes_showcase`
//!
//! This example demonstrates the GPU blend state integration implemented in GUP-027,
//! showcasing how different blend modes affect visual composition.

use gup::examples::run_blend_modes_showcase;

#[tokio::main]
async fn main() {
    match run_blend_modes_showcase().await {
        Ok(()) => {
            println!("\n✨ Example completed successfully!");
        }
        Err(e) => {
            eprintln!("❌ Example failed: {e}");
            std::process::exit(1);
        }
    }
}
